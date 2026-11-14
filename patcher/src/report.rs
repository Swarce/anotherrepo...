//! The patch report written next to every output APK.

#[derive(Default)]
pub struct Report {
    pub input: String,
    pub input_sha256: String,
    pub build: String,
    pub lines: Vec<(String, String, String, String)>, // id, description, status, detail
    pub stats: String,
    pub output: String,
    pub output_sha256: String,
    pub schemes: Vec<String>,
    pub cert_sha256: String,
    pub checks: Vec<String>,
}

impl Report {
    pub fn render(&self) -> String {
        let mut s = String::new();
        s += &format!(
            "JDR patch report (patcher {})\n\n",
            env!("CARGO_PKG_VERSION")
        );
        s += &format!(
            "input:   {}\n  sha256 {}\n  build  {}\n\n",
            self.input, self.input_sha256, self.build
        );
        s += "patches:\n";
        for (id, desc, status, detail) in &self.lines {
            s += &format!("  [{status:>8}] {id:<12} {desc}\n             {detail}\n");
        }
        s += &format!("\nzip: {}\n\n", self.stats);
        s += &format!(
            "output:  {}\n  sha256 {}\n  signed {}\n  cert   {}\n\n",
            self.output,
            self.output_sha256,
            self.schemes.join("+"),
            self.cert_sha256
        );
        s += "post-checks:\n";
        for c in &self.checks {
            s += &format!("  ok  {c}\n");
        }
        s
    }
}
