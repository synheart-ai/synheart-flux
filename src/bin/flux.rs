//! Flux CLI - Command-line interface for Synheart Flux
//!
//! Commands:
//! - transform: Process raw events into HSI output (batch mode)
//! - run: Process streaming input from stdin (streaming mode)
//! - validate: Validate raw event schema
//! - doctor: Diagnose pipeline health and configuration

use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use synheart_flux::schema::{RawEvent, RawEventAdapter, SCHEMA_VERSION};
use synheart_flux::pipeline::FluxProcessor;
use synheart_flux::types::HsiPayload;
use synheart_flux::{FLUX_VERSION, PRODUCER_NAME};

/// Flux - On-device compute engine for HSI-compliant human state signals
#[derive(Parser)]
#[command(name = "flux")]
#[command(author = "Synheart AI Inc")]
#[command(version = FLUX_VERSION)]
#[command(about = "Transform wearable data into HSI signals", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Transform raw events into HSI output (batch mode)
    Transform {
        /// Input file path (use - for stdin)
        #[arg(short, long)]
        input: PathBuf,

        /// Output file path (use - for stdout)
        #[arg(short, long)]
        output: PathBuf,

        /// Input format
        #[arg(long, default_value = "ndjson")]
        input_format: InputFormat,

        /// Output format
        #[arg(long, default_value = "ndjson")]
        output_format: OutputFormat,

        /// User timezone (IANA format, e.g., "America/New_York")
        #[arg(long, default_value = "UTC")]
        timezone: String,

        /// Device ID for provenance tracking
        #[arg(long, default_value = "unknown")]
        device_id: String,

        /// Baseline window in days
        #[arg(long, default_value = "14")]
        baseline_days: usize,

        /// Load baselines from file
        #[arg(long)]
        load_baselines: Option<PathBuf>,

        /// Save baselines to file after processing
        #[arg(long)]
        save_baselines: Option<PathBuf>,
    },

    /// Process streaming input from stdin (streaming mode)
    Run {
        /// Output format
        #[arg(long, default_value = "ndjson")]
        output_format: OutputFormat,

        /// User timezone
        #[arg(long, default_value = "UTC")]
        timezone: String,

        /// Device ID
        #[arg(long, default_value = "unknown")]
        device_id: String,

        /// Baseline window in days
        #[arg(long, default_value = "14")]
        baseline_days: usize,

        /// Load baselines from file
        #[arg(long)]
        load_baselines: Option<PathBuf>,

        /// Save baselines to file on exit
        #[arg(long)]
        save_baselines: Option<PathBuf>,

        /// Flush output after each record
        #[arg(long, default_value = "true")]
        flush: bool,
    },

    /// Validate raw event schema
    Validate {
        /// Input file path (use - for stdin)
        #[arg(short, long)]
        input: PathBuf,

        /// Input format
        #[arg(long, default_value = "ndjson")]
        input_format: InputFormat,

        /// Output validation report as JSON
        #[arg(long)]
        json: bool,
    },

    /// Diagnose pipeline health and configuration
    Doctor {
        /// Check baselines file
        #[arg(long)]
        baselines: Option<PathBuf>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Print schema information
    Schema {
        /// Schema to print (input or output)
        #[arg(value_enum)]
        schema_type: SchemaType,

        /// Output as JSON schema
        #[arg(long)]
        json_schema: bool,
    },
}

#[derive(Clone, ValueEnum)]
enum InputFormat {
    /// Newline-delimited JSON (one event per line)
    Ndjson,
    /// JSON array of events
    Json,
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    /// Newline-delimited JSON (one HSI record per line)
    Ndjson,
    /// JSON array of HSI records
    Json,
    /// Pretty-printed JSON
    JsonPretty,
}

#[derive(Clone, ValueEnum)]
enum SchemaType {
    /// Input schema (wear.raw_event.v1)
    Input,
    /// Output schema (hsi.snapshot.v1)
    Output,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", serde_json::to_string(&CliError::from(e)).unwrap_or_else(|_| "Unknown error".to_string()));
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), FluxCliError> {
    match cli.command {
        Commands::Transform {
            input,
            output,
            input_format,
            output_format,
            timezone,
            device_id,
            baseline_days,
            load_baselines,
            save_baselines,
        } => {
            cmd_transform(
                &input,
                &output,
                input_format,
                output_format,
                &timezone,
                &device_id,
                baseline_days,
                load_baselines.as_deref(),
                save_baselines.as_deref(),
            )
        }

        Commands::Run {
            output_format,
            timezone,
            device_id,
            baseline_days,
            load_baselines,
            save_baselines,
            flush,
        } => {
            cmd_run(
                output_format,
                &timezone,
                &device_id,
                baseline_days,
                load_baselines.as_deref(),
                save_baselines.as_deref(),
                flush,
            )
        }

        Commands::Validate {
            input,
            input_format,
            json,
        } => cmd_validate(&input, input_format, json),

        Commands::Doctor { baselines, json } => cmd_doctor(baselines.as_deref(), json),

        Commands::Schema { schema_type, json_schema } => cmd_schema(schema_type, json_schema),
    }
}

fn cmd_transform(
    input: &PathBuf,
    output: &PathBuf,
    input_format: InputFormat,
    output_format: OutputFormat,
    timezone: &str,
    device_id: &str,
    baseline_days: usize,
    load_baselines: Option<&std::path::Path>,
    save_baselines: Option<&std::path::Path>,
) -> Result<(), FluxCliError> {
    // Read input
    let input_data = if input.to_string_lossy() == "-" {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer
    } else {
        fs::read_to_string(input)?
    };

    // Parse events
    let events = match input_format {
        InputFormat::Ndjson => RawEventAdapter::parse_ndjson(&input_data)?,
        InputFormat::Json => RawEventAdapter::parse_array(&input_data)?,
    };

    if events.is_empty() {
        return Err(FluxCliError::NoEvents);
    }

    // Convert to canonical signals
    let canonical_signals = RawEventAdapter::to_canonical(&events, timezone, device_id)?;

    if canonical_signals.is_empty() {
        return Err(FluxCliError::NoSignals);
    }

    // Create processor with baselines
    let mut processor = FluxProcessor::with_baseline_window(baseline_days);

    // Load existing baselines if provided
    if let Some(baselines_path) = load_baselines {
        let baselines_json = fs::read_to_string(baselines_path)?;
        processor.load_baselines(&baselines_json)?;
    }

    // Process each day's signals through the pipeline
    let mut hsi_outputs: Vec<HsiPayload> = Vec::new();

    for signals in canonical_signals {
        // Convert signals to vendor JSON format for processing
        // (This is a bridge until we refactor the pipeline to accept canonical directly)
        let vendor_json = convert_canonical_to_vendor_json(&signals)?;

        let hsi_jsons = match signals.vendor {
            synheart_flux::types::Vendor::Whoop => {
                processor.process_whoop(&vendor_json, timezone, device_id)?
            }
            synheart_flux::types::Vendor::Garmin => {
                processor.process_garmin(&vendor_json, timezone, device_id)?
            }
        };

        // Parse each HSI JSON output
        for hsi_json in hsi_jsons {
            let hsi_record: HsiPayload = serde_json::from_str(&hsi_json)?;
            hsi_outputs.push(hsi_record);
        }
    }

    // Save baselines if requested
    if let Some(baselines_path) = save_baselines {
        let baselines_json = processor.save_baselines()?;
        fs::write(baselines_path, baselines_json)?;
    }

    // Write output
    let output_data = format_output(&hsi_outputs, &output_format)?;

    if output.to_string_lossy() == "-" {
        print!("{}", output_data);
    } else {
        fs::write(output, output_data)?;
    }

    Ok(())
}

fn cmd_run(
    output_format: OutputFormat,
    timezone: &str,
    device_id: &str,
    baseline_days: usize,
    load_baselines: Option<&std::path::Path>,
    save_baselines: Option<&std::path::Path>,
    flush: bool,
) -> Result<(), FluxCliError> {
    let mut processor = FluxProcessor::with_baseline_window(baseline_days);

    // Load existing baselines if provided
    if let Some(baselines_path) = load_baselines {
        let baselines_json = fs::read_to_string(baselines_path)?;
        processor.load_baselines(&baselines_json)?;
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut event_buffer: Vec<RawEvent> = Vec::new();
    let mut current_date: Option<String> = None;

    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Parse the event
        let event: RawEvent = serde_json::from_str(trimmed).map_err(|e| {
            FluxCliError::ParseError(format!("Failed to parse event: {}", e))
        })?;

        // Validate the event
        event.validate()?;

        // Check if we need to flush the buffer (date changed)
        let event_date = event.timestamp.format("%Y-%m-%d").to_string();

        if let Some(ref date) = current_date {
            if &event_date != date && !event_buffer.is_empty() {
                // Process buffered events
                let output = process_event_buffer(
                    &mut processor,
                    &event_buffer,
                    timezone,
                    device_id,
                    &output_format,
                )?;

                write!(stdout, "{}", output)?;
                if flush {
                    stdout.flush()?;
                }

                event_buffer.clear();
            }
        }

        current_date = Some(event_date);
        event_buffer.push(event);
    }

    // Process remaining events
    if !event_buffer.is_empty() {
        let output = process_event_buffer(
            &mut processor,
            &event_buffer,
            timezone,
            device_id,
            &output_format,
        )?;

        write!(stdout, "{}", output)?;
        stdout.flush()?;
    }

    // Save baselines if requested
    if let Some(baselines_path) = save_baselines {
        let baselines_json = processor.save_baselines()?;
        fs::write(baselines_path, baselines_json)?;
    }

    Ok(())
}

fn process_event_buffer(
    processor: &mut FluxProcessor,
    events: &[RawEvent],
    timezone: &str,
    device_id: &str,
    output_format: &OutputFormat,
) -> Result<String, FluxCliError> {
    let canonical_signals = RawEventAdapter::to_canonical(events, timezone, device_id)?;

    let mut hsi_outputs: Vec<HsiPayload> = Vec::new();

    for signals in canonical_signals {
        let vendor_json = convert_canonical_to_vendor_json(&signals)?;

        let hsi_jsons = match signals.vendor {
            synheart_flux::types::Vendor::Whoop => {
                processor.process_whoop(&vendor_json, timezone, device_id)?
            }
            synheart_flux::types::Vendor::Garmin => {
                processor.process_garmin(&vendor_json, timezone, device_id)?
            }
        };

        for hsi_json in hsi_jsons {
            let hsi_record: HsiPayload = serde_json::from_str(&hsi_json)?;
            hsi_outputs.push(hsi_record);
        }
    }

    format_output(&hsi_outputs, output_format)
}

fn cmd_validate(
    input: &PathBuf,
    input_format: InputFormat,
    json: bool,
) -> Result<(), FluxCliError> {
    // Read input
    let input_data = if input.to_string_lossy() == "-" {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer
    } else {
        fs::read_to_string(input)?
    };

    // Parse events
    let events = match input_format {
        InputFormat::Ndjson => RawEventAdapter::parse_ndjson(&input_data)?,
        InputFormat::Json => RawEventAdapter::parse_array(&input_data)?,
    };

    // Validate each event
    let results = RawEventAdapter::validate_events(&events);

    let report = ValidationReport {
        total_events: events.len(),
        valid_events: events.len() - results.len(),
        invalid_events: results.len(),
        errors: results
            .iter()
            .map(|r| ValidationErrorDetail {
                index: r.index,
                event_id: r.event_id.clone(),
                error: r.result.as_ref().map(|e| e.to_string()).unwrap_or_default(),
            })
            .collect(),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Validation Report");
        println!("=================");
        println!("Total events:   {}", report.total_events);
        println!("Valid events:   {}", report.valid_events);
        println!("Invalid events: {}", report.invalid_events);

        if !report.errors.is_empty() {
            println!("\nErrors:");
            for err in &report.errors {
                println!(
                    "  - Event {} (index {}): {}",
                    err.event_id.as_deref().unwrap_or("unknown"),
                    err.index,
                    err.error
                );
            }
        }
    }

    if report.invalid_events > 0 {
        Err(FluxCliError::ValidationFailed(report.invalid_events))
    } else {
        Ok(())
    }
}

fn cmd_doctor(baselines: Option<&std::path::Path>, json: bool) -> Result<(), FluxCliError> {
    let mut checks: Vec<DoctorCheck> = Vec::new();

    // Check Flux version
    checks.push(DoctorCheck {
        name: "flux_version".to_string(),
        status: CheckStatus::Ok,
        message: format!("Flux version {}", FLUX_VERSION),
    });

    // Check schema version
    checks.push(DoctorCheck {
        name: "schema_version".to_string(),
        status: CheckStatus::Ok,
        message: format!("Input schema: {}", SCHEMA_VERSION),
    });

    // Check baselines file if provided
    if let Some(baselines_path) = baselines {
        if baselines_path.exists() {
            match fs::read_to_string(baselines_path) {
                Ok(content) => {
                    match serde_json::from_str::<serde_json::Value>(&content) {
                        Ok(value) => {
                            let days = value.get("baseline_days")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            checks.push(DoctorCheck {
                                name: "baselines".to_string(),
                                status: CheckStatus::Ok,
                                message: format!(
                                    "Baselines file valid ({} days of data)",
                                    days
                                ),
                            });
                        }
                        Err(e) => {
                            checks.push(DoctorCheck {
                                name: "baselines".to_string(),
                                status: CheckStatus::Error,
                                message: format!("Invalid baselines JSON: {}", e),
                            });
                        }
                    }
                }
                Err(e) => {
                    checks.push(DoctorCheck {
                        name: "baselines".to_string(),
                        status: CheckStatus::Error,
                        message: format!("Cannot read baselines file: {}", e),
                    });
                }
            }
        } else {
            checks.push(DoctorCheck {
                name: "baselines".to_string(),
                status: CheckStatus::Warning,
                message: "Baselines file does not exist".to_string(),
            });
        }
    }

    // Check stdin is available (for streaming mode)
    let stdin_check = if atty::is(atty::Stream::Stdin) {
        DoctorCheck {
            name: "stdin".to_string(),
            status: CheckStatus::Ok,
            message: "stdin is a TTY (interactive mode)".to_string(),
        }
    } else {
        DoctorCheck {
            name: "stdin".to_string(),
            status: CheckStatus::Ok,
            message: "stdin is a pipe (streaming mode ready)".to_string(),
        }
    };
    checks.push(stdin_check);

    let report = DoctorReport {
        producer: PRODUCER_NAME.to_string(),
        version: FLUX_VERSION.to_string(),
        checks,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Flux Doctor Report");
        println!("==================");
        println!("Producer: {}", report.producer);
        println!("Version:  {}", report.version);
        println!("\nChecks:");

        for check in &report.checks {
            let status_icon = match check.status {
                CheckStatus::Ok => "[OK]",
                CheckStatus::Warning => "[WARN]",
                CheckStatus::Error => "[ERR]",
            };
            println!("  {} {}: {}", status_icon, check.name, check.message);
        }
    }

    let has_errors = report.checks.iter().any(|c| matches!(c.status, CheckStatus::Error));
    if has_errors {
        Err(FluxCliError::DoctorFailed)
    } else {
        Ok(())
    }
}

fn cmd_schema(schema_type: SchemaType, json_schema: bool) -> Result<(), FluxCliError> {
    match schema_type {
        SchemaType::Input => {
            if json_schema {
                println!("{}", get_input_json_schema());
            } else {
                println!("Input Schema: {}", SCHEMA_VERSION);
                println!();
                println!("The wear.raw_event.v1 schema supports four record types:");
                println!();
                println!("1. signal - Individual point-in-time readings");
                println!("   - heart_rate, heart_rate_variability, resting_heart_rate");
                println!("   - respiratory_rate, spo2");
                println!("   - steps, calories, distance, active_minutes");
                println!("   - skin_temperature, weight, body_fat");
                println!();
                println!("2. session - Sleep, workout, and other sessions");
                println!("   - sleep, nap, workout, meditation, recovery");
                println!("   - Contains start_time, end_time, and flexible metrics");
                println!();
                println!("3. summary - Daily or hourly aggregates");
                println!("   - period: hourly, daily, weekly, monthly");
                println!("   - Contains date and flexible metrics");
                println!();
                println!("4. score - Vendor-computed scores");
                println!("   - recovery, strain, sleep, readiness, stress, body_battery");
                println!("   - Contains value, scale (min/max), and optional components");
                println!();
                println!("Supported providers: whoop, garmin, apple, oura, fitbit, polar, coros, suunto, samsung, withings");
            }
        }
        SchemaType::Output => {
            if json_schema {
                println!("{}", get_output_json_schema());
            } else {
                println!("Output Schema: hsi.snapshot.v1");
                println!();
                println!("HSI (Human State Interface) output contains:");
                println!();
                println!("- hsi_version: Schema version (1.0.0)");
                println!("- producer: {{ name, version, instance_id }}");
                println!("- provenance: {{ source_vendor, source_device_id, timestamps }}");
                println!("- quality: {{ coverage, freshness_sec, confidence, flags }}");
                println!("- windows: Array of daily windows containing:");
                println!("  - date, timezone");
                println!("  - sleep: {{ duration, efficiency, fragmentation, deep_ratio, rem_ratio, ... }}");
                println!("  - physiology: {{ hrv_rmssd_ms, resting_hr_bpm, spo2_percentage, ... }}");
                println!("  - activity: {{ strain_score, normalized_load, calories, steps, ... }}");
                println!("  - baseline: {{ hrv_ms, resting_hr_bpm, deviations, days_in_baseline }}");
            }
        }
    }

    Ok(())
}

// Helper functions

fn format_output(hsi_outputs: &[HsiPayload], format: &OutputFormat) -> Result<String, FluxCliError> {
    match format {
        OutputFormat::Ndjson => {
            let mut lines: Vec<String> = Vec::new();
            for hsi in hsi_outputs {
                lines.push(serde_json::to_string(hsi)?);
            }
            Ok(lines.join("\n") + "\n")
        }
        OutputFormat::Json => {
            Ok(serde_json::to_string(hsi_outputs)?)
        }
        OutputFormat::JsonPretty => {
            Ok(serde_json::to_string_pretty(hsi_outputs)?)
        }
    }
}

fn convert_canonical_to_vendor_json(
    signals: &synheart_flux::types::CanonicalWearSignals,
) -> Result<String, FluxCliError> {
    // Convert canonical signals back to vendor format for processing
    // This is a temporary bridge until we refactor the pipeline

    match signals.vendor {
        synheart_flux::types::Vendor::Whoop => {
            let payload = serde_json::json!({
                "sleep": [{
                    "id": 1,
                    "start": signals.sleep.start_time.map(|t| t.to_rfc3339()).unwrap_or_default(),
                    "end": signals.sleep.end_time.map(|t| t.to_rfc3339()).unwrap_or_default(),
                    "score": {
                        "stage_summary": {
                            "total_in_bed_time_milli": signals.sleep.time_in_bed_minutes.map(|m| (m * 60_000.0) as i64).unwrap_or(0),
                            "total_awake_time_milli": signals.sleep.awake_minutes.map(|m| (m * 60_000.0) as i64).unwrap_or(0),
                            "total_light_sleep_time_milli": signals.sleep.light_sleep_minutes.map(|m| (m * 60_000.0) as i64).unwrap_or(0),
                            "total_slow_wave_sleep_time_milli": signals.sleep.deep_sleep_minutes.map(|m| (m * 60_000.0) as i64).unwrap_or(0),
                            "total_rem_sleep_time_milli": signals.sleep.rem_sleep_minutes.map(|m| (m * 60_000.0) as i64).unwrap_or(0),
                            "total_sleep_time_milli": signals.sleep.total_sleep_minutes.map(|m| (m * 60_000.0) as i64).unwrap_or(0),
                            "disturbance_count": signals.sleep.awakenings.unwrap_or(0)
                        },
                        "sleep_performance_percentage": signals.sleep.vendor_sleep_score,
                        "respiratory_rate": signals.sleep.respiratory_rate
                    }
                }],
                "recovery": [{
                    "cycle_id": 1,
                    "created_at": signals.observed_at.to_rfc3339(),
                    "score": {
                        "recovery_score": signals.recovery.vendor_recovery_score,
                        "resting_heart_rate": signals.recovery.resting_hr_bpm,
                        "hrv_rmssd_milli": signals.recovery.hrv_rmssd_ms,
                        "spo2_percentage": signals.recovery.spo2_percentage,
                        "skin_temp_celsius": signals.recovery.skin_temp_deviation_c
                    }
                }],
                "cycle": [{
                    "id": 1,
                    "start": signals.sleep.end_time.map(|t| t.to_rfc3339()).unwrap_or_default(),
                    "end": signals.observed_at.to_rfc3339(),
                    "score": {
                        "strain": signals.activity.vendor_strain_score,
                        "kilojoule": signals.activity.calories.map(|c| c / 0.239006),
                        "average_heart_rate": signals.activity.average_hr_bpm,
                        "max_heart_rate": signals.activity.max_hr_bpm
                    }
                }]
            });
            Ok(serde_json::to_string(&payload)?)
        }
        synheart_flux::types::Vendor::Garmin => {
            let payload = serde_json::json!({
                "dailies": [{
                    "calendarDate": signals.date,
                    "totalSteps": signals.activity.steps,
                    "totalDistanceMeters": signals.activity.distance_meters.map(|d| d as i64),
                    "totalKilocalories": signals.activity.calories.map(|c| c as i32),
                    "activeKilocalories": signals.activity.active_calories.map(|c| c as i32),
                    "restingHeartRate": signals.recovery.resting_hr_bpm.map(|h| h as i32),
                    "restingHeartRateHrv": signals.recovery.hrv_rmssd_ms,
                    "averageHeartRate": signals.activity.average_hr_bpm.map(|h| h as i32),
                    "maxHeartRate": signals.activity.max_hr_bpm.map(|h| h as i32),
                    "avgSpo2Value": signals.recovery.spo2_percentage,
                    "bodyBatteryChargedValue": signals.recovery.vendor_recovery_score.map(|r| r as i32),
                    "trainingLoadBalance": signals.activity.vendor_strain_score,
                    "moderateIntensityMinutes": signals.activity.active_minutes.map(|m| (m * 0.7) as i32),
                    "vigorousIntensityMinutes": signals.activity.active_minutes.map(|m| (m * 0.3) as i32)
                }],
                "sleep": [{
                    "calendarDate": signals.date,
                    "sleepStartTimestampGmt": signals.sleep.start_time.map(|t| t.timestamp_millis()),
                    "sleepEndTimestampGmt": signals.sleep.end_time.map(|t| t.timestamp_millis()),
                    "sleepTimeSeconds": signals.sleep.total_sleep_minutes.map(|m| (m * 60.0) as i64),
                    "awakeSleepSeconds": signals.sleep.awake_minutes.map(|m| (m * 60.0) as i64),
                    "lightSleepSeconds": signals.sleep.light_sleep_minutes.map(|m| (m * 60.0) as i64),
                    "deepSleepSeconds": signals.sleep.deep_sleep_minutes.map(|m| (m * 60.0) as i64),
                    "remSleepSeconds": signals.sleep.rem_sleep_minutes.map(|m| (m * 60.0) as i64),
                    "awakeCount": signals.sleep.awakenings,
                    "avgSleepRespiration": signals.sleep.respiratory_rate,
                    "sleepScores": {
                        "overallScore": signals.sleep.vendor_sleep_score
                    }
                }]
            });
            Ok(serde_json::to_string(&payload)?)
        }
    }
}

fn get_input_json_schema() -> String {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://synheart.ai/schemas/wear.raw_event.v1.json",
        "title": "wear.raw_event.v1",
        "description": "Synheart wearable raw event schema",
        "type": "object",
        "required": ["schema_version", "timestamp", "source", "record_type", "payload"],
        "properties": {
            "schema_version": {
                "type": "string",
                "const": "wear.raw_event.v1"
            },
            "event_id": { "type": "string" },
            "timestamp": { "type": "string", "format": "date-time" },
            "source": {
                "type": "object",
                "required": ["provider"],
                "properties": {
                    "provider": { "type": "string" },
                    "device_model": { "type": "string" },
                    "device_id": { "type": "string" },
                    "firmware_version": { "type": "string" }
                }
            },
            "user_id": { "type": "string" },
            "record_type": {
                "type": "string",
                "enum": ["signal", "session", "summary", "score"]
            },
            "payload": { "type": "object" },
            "context": {
                "type": "object",
                "properties": {
                    "activity_type": { "type": "string" },
                    "session_id": { "type": "string" },
                    "timezone": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } }
                }
            },
            "vendor_raw": { "type": "object" }
        }
    }).to_string()
}

fn get_output_json_schema() -> String {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://synheart.ai/schemas/hsi.snapshot.v1.json",
        "title": "hsi.snapshot.v1",
        "description": "Synheart HSI output schema",
        "type": "object",
        "required": ["hsi_version", "producer", "provenance", "quality", "windows"],
        "properties": {
            "hsi_version": { "type": "string" },
            "producer": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "version": { "type": "string" },
                    "instance_id": { "type": "string" }
                }
            },
            "provenance": {
                "type": "object",
                "properties": {
                    "source_vendor": { "type": "string" },
                    "source_device_id": { "type": "string" },
                    "observed_at_utc": { "type": "string" },
                    "computed_at_utc": { "type": "string" }
                }
            },
            "quality": {
                "type": "object",
                "properties": {
                    "coverage": { "type": "number" },
                    "freshness_sec": { "type": "integer" },
                    "confidence": { "type": "number" },
                    "flags": { "type": "array", "items": { "type": "string" } }
                }
            },
            "windows": {
                "type": "array",
                "items": { "type": "object" }
            }
        }
    }).to_string()
}

// Error types

#[derive(Debug)]
enum FluxCliError {
    Io(io::Error),
    Parse(synheart_flux::ComputeError),
    Json(serde_json::Error),
    Validation(synheart_flux::schema::ValidationError),
    NoEvents,
    NoSignals,
    ValidationFailed(usize),
    DoctorFailed,
    ParseError(String),
}

impl From<io::Error> for FluxCliError {
    fn from(e: io::Error) -> Self {
        FluxCliError::Io(e)
    }
}

impl From<synheart_flux::ComputeError> for FluxCliError {
    fn from(e: synheart_flux::ComputeError) -> Self {
        FluxCliError::Parse(e)
    }
}

impl From<serde_json::Error> for FluxCliError {
    fn from(e: serde_json::Error) -> Self {
        FluxCliError::Json(e)
    }
}

impl From<synheart_flux::schema::ValidationError> for FluxCliError {
    fn from(e: synheart_flux::schema::ValidationError) -> Self {
        FluxCliError::Validation(e)
    }
}

#[derive(serde::Serialize)]
struct CliError {
    code: String,
    message: String,
    hint: Option<String>,
}

impl From<FluxCliError> for CliError {
    fn from(e: FluxCliError) -> Self {
        match e {
            FluxCliError::Io(e) => CliError {
                code: "IO_ERROR".to_string(),
                message: e.to_string(),
                hint: Some("Check file paths and permissions".to_string()),
            },
            FluxCliError::Parse(e) => CliError {
                code: "PARSE_ERROR".to_string(),
                message: e.to_string(),
                hint: Some("Ensure input matches wear.raw_event.v1 schema".to_string()),
            },
            FluxCliError::Json(e) => CliError {
                code: "JSON_ERROR".to_string(),
                message: e.to_string(),
                hint: Some("Check JSON syntax".to_string()),
            },
            FluxCliError::Validation(e) => CliError {
                code: "VALIDATION_ERROR".to_string(),
                message: e.to_string(),
                hint: Some("Run 'flux validate' for details".to_string()),
            },
            FluxCliError::NoEvents => CliError {
                code: "NO_EVENTS".to_string(),
                message: "No events found in input".to_string(),
                hint: Some("Ensure input file is not empty".to_string()),
            },
            FluxCliError::NoSignals => CliError {
                code: "NO_SIGNALS".to_string(),
                message: "No processable signals found".to_string(),
                hint: Some("Check that events contain valid signal/session/summary data".to_string()),
            },
            FluxCliError::ValidationFailed(count) => CliError {
                code: "VALIDATION_FAILED".to_string(),
                message: format!("{} events failed validation", count),
                hint: Some("Fix validation errors and retry".to_string()),
            },
            FluxCliError::DoctorFailed => CliError {
                code: "DOCTOR_FAILED".to_string(),
                message: "One or more health checks failed".to_string(),
                hint: Some("Review the doctor report for details".to_string()),
            },
            FluxCliError::ParseError(msg) => CliError {
                code: "PARSE_ERROR".to_string(),
                message: msg,
                hint: Some("Check input format".to_string()),
            },
        }
    }
}

// Report types

#[derive(serde::Serialize)]
struct ValidationReport {
    total_events: usize,
    valid_events: usize,
    invalid_events: usize,
    errors: Vec<ValidationErrorDetail>,
}

#[derive(serde::Serialize)]
struct ValidationErrorDetail {
    index: usize,
    event_id: Option<String>,
    error: String,
}

#[derive(serde::Serialize)]
struct DoctorReport {
    producer: String,
    version: String,
    checks: Vec<DoctorCheck>,
}

#[derive(serde::Serialize)]
struct DoctorCheck {
    name: String,
    status: CheckStatus,
    message: String,
}

#[derive(serde::Serialize)]
enum CheckStatus {
    Ok,
    Warning,
    Error,
}
