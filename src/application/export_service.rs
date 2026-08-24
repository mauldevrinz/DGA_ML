#![allow(dead_code)]
use anyhow::{Result, Context};
use std::path::Path;
use crate::domain::models::SensorReading;

pub struct ExportService;

impl ExportService {
    pub fn export_sensor_data<P: AsRef<Path>>(path: P, data: &[SensorReading]) -> Result<()> {
        let mut wtr = csv::Writer::from_path(path)
            .context("Failed to create CSV file")?;

        // Write header
        wtr.write_record(&[
            "timestamp_ms",
            "mos_01", "mos_02", "mos_03", "mos_04", "mos_05", "mos_06", "mos_07",
            "mos_08", "mos_09", "mos_10", "mos_11", "mos_12", "mos_13", "mos_14",
            "ndir", "sht30_temp", "sht30_humidity"
        ])?;

        // Write records
        for reading in data {
            let mut record = Vec::with_capacity(18);
            record.push(reading.timestamp_ms.to_string());
            for v in &reading.values {
                record.push(v.to_string());
            }
            wtr.write_record(&record)?;
        }

        wtr.flush()?;
        Ok(())
    }
}
