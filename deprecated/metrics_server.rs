use may_minihttp::{HttpServer, HttpService, Request, Response};
use prometheus::Encoder;
use std::io;

#[derive(Clone)]
struct MetricsService;

impl HttpService for MetricsService {
    fn call(&mut self, req: Request, res: &mut Response) -> io::Result<()> {
        if req.path() == "/metrics" {
            let encoder = prometheus::TextEncoder::new();
            // Use prometheus::gather() instead of trying to call gather() on METRICS.exporter
            let metric_families = prometheus::gather();
            let mut buffer = vec![];
            encoder.encode(&metric_families, &mut buffer).unwrap();
            // Fix header function call format (key, value)
            res.header("Content-Type: text/plain; charset=utf-8");
            // Convert to a static string that lives for the program duration
            res.status_code(200, "OK");
        } else {
            res.status_code(404, "Not Found");
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let host_port = std::env::var("HOST_PORT").unwrap_or_else(|_| "127.0.0.1:9898".to_string());
    let server = HttpServer(MetricsService)
        .start(&host_port)
        .map_err(|e| anyhow::anyhow!("Failed to start server: {}", e))?;
    println!("📊 Metrics server running at http://{}/metrics", host_port);
    server
        .join()
        .map_err(|e| anyhow::anyhow!("Server encountered an error: {:?}", e))?;
    Ok(())
}
