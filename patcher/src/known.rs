//! Builds the patcher has been verified against. Every patch site is fingerprinted here;
//! if the player's copy does not match, that patch refuses (nothing half-patches).

/// What a stubbed method becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stub {
    /// `ret` (a `void` method that does nothing).
    Ret,
    /// `mov w0, #0; ret` (a `bool` method that answers false).
    RetFalse,
}

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
    /// File offset and SHA-256 (32-byte window) of `GrowthbeatBehaviour.Awake`, `.SendTag` and
    /// `.SendEvent` in libil2cpp.so (the `no-growthbeat` patch makes each return at once).
    pub growthbeat_fns: [(usize, &'static str); 3],
    /// File offset and SHA-256 (`patches::TOWN_DNS_LEN`-byte window) of the nine instructions in
    /// `Jjs.Socket.SocketBase.Connect` that swap the server's address for
    /// `Dns.GetHostEntry(ip).HostName` (the `town-direct-ip` patch removes them).
    pub town_dns_site: (usize, &'static str),
    /// SDK entry points the `debloat` patch stubs: file offset, SHA-256 (32-byte window), the
    /// method, and what it becomes.
    pub sdk_stubs: &'static [(usize, &'static str, &'static str, Stub)],
    /// File offset and SHA-256 (32-byte window) of the `mvn w8, w20` in
    /// `TitleUIController.SetupGooglePlayGamesLoginButton` (`SetActive(!Authenticated)`); `debloat`
    /// makes it `mov w8, #0`, so the title's Play Games button stays hidden.
    pub title_play_games_button: (usize, &'static str),
    /// SHA-256 of the binary `AndroidManifest.xml`.
    pub manifest_sha256: &'static str,
    /// SHA-256 of the Resources file holding `Text/ClientMes_JP` (`clientmes::ENTRY`).
    pub clientmes_sha256: &'static str,
    /// SHA-256 of each scene built into the APK (`assets/bin/Data/level<N>`) that holds Japanese
    /// `UI.Text` labels (`english-apk-text` rewrites them).
    pub apk_text_scenes: &'static [(&'static str, &'static str)],
    /// Player-visible C# string literals (index, what it is) that `english-apk-text` translates
    /// and `jdr-tool l10n export --apk` offers. Debug-only literals are left out on purpose.
    pub ui_literals: &'static [(usize, &'static str)],
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
    growthbeat_fns: [
        (
            0x2071f7c,
            "e525d7fbc822f927cd7b5aa7508aee7becae77470f60b204e19f928275d2ad51",
        ),
        (
            0x20720a0,
            "3509a00ceedfbd591919d9085f72046b2be5edc3acad2d1c9e8def7c13a5673a",
        ),
        (
            0x2072158,
            "b43cd6d0bd5047950561df5ff70c6951634585af5a8a88eb1ba568200e8169f4",
        ),
    ],
    town_dns_site: (
        0xcadda4,
        "6be3fa8bf1014f7372682e9fb6ce85b8de4982385331725954179cc582f6c64c",
    ),
    sdk_stubs: &[
        // SmartBeat (crash reporting): never initialised, its native library never loaded
        (
            0x107ed5c,
            "a777bd87ab3365c9a4e593d40d3c6ac4910eed4ddc8e294e27f62f944e05ebf3",
            "SmartBeat.SmartBeatBehaviour.Awake",
            Stub::Ret,
        ),
        (
            0x107f1a4,
            "a76c712dac692395732159546bdd69b11181044ccd2b151420bb8c97c1f98d9c",
            "SmartBeat.SmartBeatBehaviour.OnApplicationPause",
            Stub::Ret,
        ),
        (
            0x107f208,
            "838837cf48802fc38021471f452627065fa2d09231b99584cb8e3434ca69bfba",
            "SmartBeat.SmartBeatBehaviour.Update",
            Stub::Ret,
        ),
        // Adjust (install and event tracking)
        (
            0x18f8af0,
            "b13f38f0ef664d8d22563b8c692f397ed6029becc48754b55c971ffb20e7cba5",
            "com.adjust.sdk.Adjust.start",
            Stub::Ret,
        ),
        (
            0x18f93f0,
            "ff7cde1e4d6ba0d85e88f17486a254fd7ec96ae4b3589c59694eae3d81a571d0",
            "com.adjust.sdk.Adjust.trackEvent",
            Stub::Ret,
        ),
        (
            0x18f9100,
            "47b6003469e9595ccb80cf7147a7f406a537a3983d681b3b11e905dd72552494",
            "com.adjust.sdk.Adjust.OnApplicationPause",
            Stub::Ret,
        ),
        // Google Play Games: never activated (UnityEngine.Social keeps Unity's own local
        // platform), so libgpg is never loaded; no automatic sign-in
        (
            0x10612b0,
            "697178122da20031e8db7e9752f570d8eb4c410d5fe82d5bd60f0399223e1b3d",
            "ScSystem.ScSocial.AchievementManager.Init",
            Stub::Ret,
        ),
        (
            0x20602f8,
            "ad174ab9b4641e58891d790bef0a6c38ed06bdeeb7cab2073000269eb69e4c27",
            "Jjs.AchievementUtil.get_NeedAutoSignIn",
            Stub::RetFalse,
        ),
    ],
    title_play_games_button: (
        0xb90da4,
        "ccaddce7da666ff67c2b76ac08db6d4760ab4d399a7f34520135b37707ff56b8",
    ),
    manifest_sha256: "a1d3a440e4e6ba6cc1bf6a8fa91cc03fd948058d55630c9aa41820dc11061257",
    clientmes_sha256: "eb62e1d6f6c8dcd3d186ffb0af05d2c0f02ff55594651ae5b72ad65e5b751501",
    apk_text_scenes: &[
        // Load/LoadResidentUI: the loading overlay ("Downloading", Wi-Fi notice, "Connecting")
        (
            "level2",
            "7be0a7596709140ef070e21d8ae94a3fd47e4d2a3d904c0166d95bdb36370b28",
        ),
        // Title/CacheDeleteUI
        (
            "level6",
            "44fe5de2a02d79d69eec3582560c827e97990a9ed249a2ab837973609b0888cf",
        ),
        // Title/ContactUI
        (
            "level7",
            "30ab9c2dd68f144a27a4864daaf8de4e96f6788455e0e2431be22f516d5f127f",
        ),
        // Title/SelectSaveDataUI
        (
            "level9",
            "f4a7c54e7c3c573dced9e4c227596043c645342e258bcd098594148be3d2daa5",
        ),
        // Title/TakeOverUI
        (
            "level10",
            "d2a1168845bd3ae60423a1455d9f8f8a7894870f22f73f496f07784751e69c09",
        ),
        // Title/TitleUI (user ID, app version)
        (
            "level11",
            "90f86847b035aacd4a1d78332fe1e653b696ea1a01ad01c76d4f79b24f1b87e9",
        ),
        // Title/ResourceDownloadTitleUI (first download)
        (
            "level12",
            "b5585a6aa8b13917fb18310d89ef32ec3819037bbdda82689af334556bfa566b",
        ),
        // Resident/CheckVersionUI
        (
            "level14",
            "ed66c8d8286742fd8397db06afd77607d7d5f517f9e4ecf28f64c9d742927593",
        ),
        // Resident/Error
        (
            "level15",
            "cbd2157b81fce8b4a8a1aeed560808cc52dba02cc1e464f90b9f15e81f4b4ac7",
        ),
        // Resident/MultiWindowUI
        (
            "level16",
            "e5a894ed71d650cb479bffbffa32ba6fdc7c8649d9ff827ed11bf0a59b724e7b",
        ),
        // Resident/WebViewUI
        (
            "level20",
            "b828d7fba83c8c8bad4ac31a9594a4d03c87eff7d8b518f34b28cf4fe0db3bf1",
        ),
    ],
    ui_literals: &[
        (
            326,
            "AffectTextUtil: placeholder shown when a skill/statue text substitution fails",
        ),
        (
            1052,
            "BuyingHistoryTextController.SetText: currency suffix after a price",
        ),
        (
            2624,
            "DtSetAbilityController: tab label (leader and support abilities)",
        ),
        (2625, "DtSetAbilityController: tab label (statues)"),
        (
            2851,
            "EventHomeUIController.UpdateUiRaidBoss: suffix after the raid boss end date",
        ),
        (
            3194,
            "FriendBannerController.UpdateButtonVisual: button, remove a follower",
        ),
        (
            3195,
            "FriendBannerController.UpdateButtonVisual: button, cancel the removal",
        ),
        (
            3196,
            "FriendBannerController.UpdateButtonVisual: button, unfollow",
        ),
        (
            3197,
            "FriendBannerController.UpdateButtonVisual: button, follow",
        ),
        (
            4388,
            "InAppPurchaseManager.consumePurchaseFailedEvent: message prefix",
        ),
        (
            4389,
            "InAppPurchaseManager.purchaseFailedEvent: message prefix",
        ),
        (4697, "LoadLogUIController: saving indicator"),
        (
            5655,
            "MiniGame2FlowModeCheck.onUpdate: game mode line, before the mode name",
        ),
        (
            5656,
            "MiniGame2FlowModeCheck.onUpdate: game mode line, after the mode name",
        ),
        (
            6806,
            "PvPHomeData.CalcPvPTimeLeft: days left (format, {0} = days)",
        ),
        (
            6811,
            "PvPHomeData.SetPlayerInformation: arena rank (format, {0} = rank)",
        ),
        (
            6813,
            "PvPHomeData.SetPlayerInformation: rank suffix after the no-rank text",
        ),
        (
            7352,
            "ResourceDownloadUIController: prefix before the download size",
        ),
        (7355, "ResourceDownloadUIController.Start: placeholder text"),
        (7870, "generic error message"),
        (
            7878,
            "SignboardBannerController.Init: banner end date (DateTime format)",
        ),
        (
            8455,
            "TextConvertUtil.GetMaxStackParam: fallback for an unsupported value",
        ),
    ],
    server_url_literal: (4137, "https://jdr.channel.or.jp"),
    http_key_literal: 4143,
}];

pub fn by_metadata(sha: &str) -> Option<&'static VerifiedBuild> {
    BUILDS.iter().find(|b| b.metadata_sha256 == sha)
}
