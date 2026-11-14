//! Builds the patcher has been verified against. Every patch site is fingerprinted here;
//! if the player's copy does not match, that patch refuses (nothing half-patches).

pub struct VerifiedBuild {
    pub version_name: &'static str,
    pub version_code: u32,
    /// SHA-256 of `assets/bin/Data/Managed/Metadata/global-metadata.dat`.
    pub metadata_sha256: &'static str,
    /// SHA-256 of `lib/arm64-v8a/libil2cpp.so`.
    pub libil2cpp_arm64_sha256: &'static str,
    /// SHA-256 of `lib/arm64-v8a/libmain.so` (its `liblog.so` dependency becomes `libjdr.so`).
    pub libmain_arm64_sha256: &'static str,
    /// SHA-256 of `lib/arm64-v8a/libunity.so`.
    pub libunity_arm64_sha256: &'static str,
    /// File offset and SHA-256 of the `UnityDefaultAllocator` constructor body that the
    /// `unity-heap` patch replaces.
    pub unity_default_alloc_ctor: (usize, &'static str),
    /// File offset and SHA-256 (32-byte window) of `SaveDataManager.LoadNonObfuscatedClearFlag`
    /// in libil2cpp.so (the `keep-files` patch makes it return true).
    pub clear_flag_fn: (usize, &'static str),
    /// SHA-256 of the binary `AndroidManifest.xml`.
    pub manifest_sha256: &'static str,
    /// SHA-256 of the Resources file holding `Text/ClientMes_JP` (`clientmes::ENTRY`).
    pub clientmes_sha256: &'static str,
    /// Index and original value of the production server URL literal.
    pub server_url_literal: (usize, &'static str),
    /// Index of the request-envelope AES key literal (`HttpConst.HTTPRijndaelKey`); read by
    /// `jdr-tool keys --apk`, never stored here.
    pub http_key_literal: usize,
}

pub const BUILDS: &[VerifiedBuild] = &[VerifiedBuild {
    version_name: "3.12.0",
    version_code: 60,
    metadata_sha256: "aeab1947a8ccfd05c8bd40d472198344d9ce3d3dd019bbcc30c906c3026aa962",
    libil2cpp_arm64_sha256: "b5653644705b9a22360608a50eed28884c7721dcb2aeb45d525fc3aa8e7c101a",
    libmain_arm64_sha256: "722bf491c8befadef7e510c0e49c619b34dd313350a3c4395091e2c2d38f5f2f",
    libunity_arm64_sha256: "cf553c2930c579518e59734a5cb96d5c42e113d873050a356068537c938f6f4e",
    unity_default_alloc_ctor: (
        0x9ff824,
        "fa948c209cfd14d2ddcdd04f4bb0b95b67d9fd8a9600412f22c69aad4c92bdfc",
    ),
    clear_flag_fn: (
        0xfd1534,
        "306d717ddc15f17dab50cb35cec6fd9b4a87e3c88da61b1ac7c8e70a0ee4d7c4",
    ),
    manifest_sha256: "a1d3a440e4e6ba6cc1bf6a8fa91cc03fd948058d55630c9aa41820dc11061257",
    clientmes_sha256: "eb62e1d6f6c8dcd3d186ffb0af05d2c0f02ff55594651ae5b72ad65e5b751501",
    server_url_literal: (4137, "https://jdr.channel.or.jp"),
    http_key_literal: 4143,
}];

pub fn by_metadata(sha: &str) -> Option<&'static VerifiedBuild> {
    BUILDS.iter().find(|b| b.metadata_sha256 == sha)
}
