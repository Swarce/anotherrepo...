//! Builds the patcher has been verified against. Every patch site is fingerprinted here;
//! if the player's copy does not match, that patch refuses (nothing half-patches).

pub struct VerifiedBuild {
    pub version_name: &'static str,
    pub version_code: u32,
    /// SHA-256 of `assets/bin/Data/Managed/Metadata/global-metadata.dat`.
    pub metadata_sha256: &'static str,
    /// SHA-256 of `lib/arm64-v8a/libil2cpp.so`.
    pub libil2cpp_arm64_sha256: &'static str,
    /// Index and original value of the production server URL literal.
    pub server_url_literal: (usize, &'static str),
}

pub const BUILDS: &[VerifiedBuild] = &[VerifiedBuild {
    version_name: "3.12.0",
    version_code: 60,
    metadata_sha256: "aeab1947a8ccfd05c8bd40d472198344d9ce3d3dd019bbcc30c906c3026aa962",
    libil2cpp_arm64_sha256: "b5653644705b9a22360608a50eed28884c7721dcb2aeb45d525fc3aa8e7c101a",
    server_url_literal: (4137, "https://jdr.channel.or.jp"),
}];

pub fn by_metadata(sha: &str) -> Option<&'static VerifiedBuild> {
    BUILDS.iter().find(|b| b.metadata_sha256 == sha)
}
