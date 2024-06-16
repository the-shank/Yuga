use rustc_hir::def_id::LocalDefId;
use rustc_middle::ty::TyCtxt;

use std::borrow::Cow;
use std::env;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use serde::Serialize;

use crate::utils;

static REPORT_LOGGER: OnceCell<Box<dyn ReportLogger>> = OnceCell::new();

/// Flushes the global report logger when dropped.
pub struct FlushHandle {
    _priv: (),
}

impl Drop for FlushHandle {
    fn drop(&mut self) {
        for logger in REPORT_LOGGER.get().iter() {
            logger.flush();
        }
    }
}

#[must_use]
pub fn init_report_logger(report_logger: Box<dyn ReportLogger>) -> FlushHandle {
    REPORT_LOGGER
        .set(report_logger)
        .map_err(|_| ())
        .expect("The logger is already initialized");

    FlushHandle { _priv: () }
}

pub fn default_report_logger() -> Box<dyn ReportLogger> {
    match env::var_os("YUGA_REPORT_PATH") {
        Some(val) => Box::new(FileLogger::new(val)),
        None => Box::new(StderrLogger::new()),
    }
}

pub fn yuga_report(report: Report) {
    REPORT_LOGGER.get().unwrap().log(report);
}

#[derive(Serialize, Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReportLevel {
    // Rank: High
    Error = 2,
    // Rank: Med
    Warning = 1,
    // Rank: Low
    Info = 0,
}

impl fmt::Display for ReportLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Serialize)]
pub struct Report {
    level: ReportLevel,
    analyzer: Cow<'static, str>,
    description: Cow<'static, str>,
    location: String,
    source: String,
}

impl Report {
    pub fn with_hir_id<T, U>(
        tcx: TyCtxt<'_>,
        level: ReportLevel,
        analyzer: T,
        description: U,
        local_def_id: LocalDefId,
    ) -> Report
    where
        T: Into<Cow<'static, str>>,
        U: Into<Cow<'static, str>>,
    {
        let hir_map = tcx.hir();
        let item_hir_id = hir_map.local_def_id_to_hir_id(local_def_id);
        let span = hir_map.span(item_hir_id);

        let source_map = tcx.sess.source_map();
        let source = source_map
            .span_to_snippet(span)
            .unwrap_or_else(|e| format!("unable to get source: {:?}", e));

        let location = source_map.span_to_diagnostic_string(span);

        Report {
            level,
            analyzer: analyzer.into(),
            description: description.into(),
            location,
            source,
        }
    }

    pub fn with_color_span<T, U>(
        tcx: TyCtxt<'_>,
        level: ReportLevel,
        analyzer: T,
        description: U,
        color_span: &utils::ColorSpan,
    ) -> Report
    where
        T: Into<Cow<'static, str>>,
        U: Into<Cow<'static, str>>,
    {
        let source_map = tcx.sess.source_map();
        let location = source_map.span_to_diagnostic_string(color_span.main_span());

        Report {
            level,
            analyzer: analyzer.into(),
            description: description.into(),
            location,
            source: color_span.to_colored_string(),
        }
    }
}

pub trait ReportLogger: Sync + Send {
    fn log(&self, report: Report);
    fn flush(&self);
}

struct StderrLogger {
    reports: Mutex<Vec<Report>>,
}

impl StderrLogger {
    fn new() -> Self {
        StderrLogger {
            reports: Mutex::new(Vec::new()),
        }
    }
}

impl ReportLogger for StderrLogger {
    fn log(&self, report: Report) {
        self.reports.lock().push(report);
    }

    fn flush(&self) {
        let stderr = std::io::stderr();
        let mut handle = stderr.lock();

        let reports = self.reports.lock();
        for report in reports.iter() {
            writeln!(
                &mut handle,
                "{} ({}): {}\n-> {}\n{}",
                &report.level,
                &report.analyzer,
                &report.description,
                &report.location,
                &report.source
            )
            .expect("stderr closed");
        }
    }
}

struct FileLogger {
    reports: Mutex<Vec<Report>>,
    file_path: PathBuf,
}

impl FileLogger {
    fn new<T>(val: T) -> Self
    where
        T: Into<PathBuf>,
    {
        FileLogger {
            reports: Mutex::new(Vec::new()),
            file_path: val.into(),
        }
    }
}

impl ReportLogger for FileLogger {
    fn log(&self, report: Report) {
        self.reports.lock().push(report);
    }

    fn flush(&self) {
        #[derive(Serialize)]
        struct Reports<'a> {
            reports: &'a [Report],
        }

        let reports = self.reports.lock();
        if !reports.is_empty() {
            let reports_ref = &*reports;
            fs::write(
                &self.file_path,
                toml::to_string_pretty(&Reports {
                    reports: reports_ref,
                })
                .expect("failed to serialize Yuga report")
                // We manually converts some characters inside toml strings
                // Match this list with test.py
                .replace("\\u001B", "\u{001B}")
                .replace("\\t", "\t"),
            )
            .expect("cannot write Yuga report to file");
        }
    }
}
