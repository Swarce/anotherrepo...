# Protocol: endpoint map

Every API call the 3.12.0 client can make, grouped by service, with the Build 01 server status.
Source: the research table of all 171 `RequestParamBase` subclasses mapped to their `get_ApiName`
path and response class (read from code, H). Response classes marked (M) were matched by name
because no `Request<>` instantiation pairs them in code.

Specs:

- [protocol/envelope.md](protocol/envelope.md): request/response wire format.
- [protocol/boot.md](protocol/boot.md): call order and required answers from launch to MyPage.
- [protocol/assets.md](protocol/assets.md): getkey, getfiledata, versionfile, CDN rules.

Research notes with evidence: [research/](research/) (01 redirect, 02 envelope, 03 progression,
04 economy, 05 live-ops, 06 native boundary, 07 integrity, 08 assets).

## Build 01 status meanings

- **implemented**: the path is in the dispatch table in `server/services/src/services.rs`.
- **logged as unknown**: no handler yet. The server answers `status:10000` with no other fields
  (the client keeps its defaults for every field), counts the call, and logs the path and request
  `param`. The 1.0 checklist requires this counter to stay at 0 in a full playthrough.

Notes:

- `/common/get_version` serves two request classes: `GetVersionRequest` (boot) and
  `HearbeatRequest` (resource-download screen; expects only the base fields).
- `/clan/search` serves three request classes (by id, by name, by policy).
- Class names are spelled as in the client (`GashPlayRequest`, `HearbeatRequest`,
  `PvpAutoFomationRequest`, `GetAllTrophyResult` is a request class).
- Paths are relative to `get_version.url`, except the first `get_version`, which goes to the
  built-in base.

| Count | |
|---|---|
| Request classes | 171 |
| Distinct paths | 168 |
| Request classes on an implemented path | 25 |
| Distinct implemented paths | 24 |

## Endpoints

### common — version check, system config, maintenance (4, 3 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/common/get_maint_sts` | `GetMaintenanceInfoRequest` | `GetMaintenanceInfoResponse` | logged as unknown |
| `/common/get_system_config` | `SystemConfigRequest` | `SystemConfigResponse` | implemented |
| `/common/get_version` | `GetVersionRequest` | `GetVersionResponse` | implemented |
| `/common/get_version` | `HearbeatRequest` | `HeartbeatResponse` | implemented |

### client — asset keys and asset URLs (2, 2 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/client/getfiledata` | `GetFileDataRequest` | `GetFileDataResponse` | implemented |
| `/client/getkey` | `GetKeyRequest` | `GetKeyResponse` | implemented |

### user — account, profile, tutorial flags, stamina (16, 8 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/user/change_backup_id` | `ChangeBackupIdRequest` | `ChangeBackupIdResponse` | logged as unknown |
| `/user/create_data_move_password` | `CreateDataMovePasswordRequest` | `CreateDataMovePasswordResponse` | logged as unknown |
| `/user/create_user` | `CreateUserRequest` | `CreateUserResponse` | implemented |
| `/user/data_move` | `DataMoveRequest` | `DataMoveResponse` | logged as unknown |
| `/user/get_loginbonus` | `GetLoginBonusRequest` | `GetLoginBonusResponse` | implemented |
| `/user/get_nonce` | `GetNonceRequest` | `GetNonceResponse` | implemented |
| `/user/get_tutorial_step` | `TutorialStepRequest` | `TutorialStepResponse` | implemented |
| `/user/login` | `LoginRequest` | `LoginResponse` | implemented |
| `/user/recover_battle_point` | `RecoveryBattlePointRequest` | `RecoveryBattlePointResponse` | logged as unknown |
| `/user/recover_stamina` | `RecoverStaminaRequest` | `RecoverStaminaResponse` | logged as unknown |
| `/user/select` | `UserSelectRequest` | `UserSelectResponse` | logged as unknown |
| `/user/tutorial` | `UpdateTutorialStepRequest` | `UpdateTutorialStepResponse` | implemented |
| `/user/update_comment` | `UpdateCommentRequest` | `UpdateCommentResponse` | logged as unknown |
| `/user/update_username` | `UpdateUserNameRequest` | `UpdateUserNameResponse` | implemented |
| `/user/user_info` | `UserInfoRequest` | `UserInfoResponse` | implemented |
| `/user/user_info_mini` | `UserInfoMiniRequest` | `UserInfoMiniResponse` | logged as unknown |

### info — notices, events, banners, menu badges (5, 4 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/info/banner_list` | `BannerListRequest` | `BannerListResponse` | implemented |
| `/info/check_forced_notify` | `CheckForcedNotifyRequest` | `CheckForcedNotifyResponse` | implemented |
| `/info/event_list` | `EventListRequest` | `EventListResponse` | implemented |
| `/info/get_menu_notify` | `GetMenuNotifyRequest` | `GetMenuNotifyResponse` | implemented |
| `/info/notify_list` | `NotifyListRequest` | `NotifyListResponse` | logged as unknown |

### shop — IAP (disabled offline) and in-game shops (9, 2 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/shop/buy_cancel` | `BuyCancelRequest` | `BuyCancelResponse` | logged as unknown |
| `/shop/buy_complete` | `BuyCompleteRequest` | `BuyCompleteResponse` | logged as unknown |
| `/shop/buy_list` | `BuyListRequest` | `BuyListResponse` | logged as unknown |
| `/shop/buy_trans` | `BuyTransRequest` | `BuyTransResponse` (M) | logged as unknown |
| `/shop/event_shop_buy` | `EventShopBuyRequest` | `EventShopBuyResponse` | implemented (Build 22; the event's currency, Build 25) |
| `/shop/event_shop_list` | `EventShopListRequest` | `EventShopListResponse` | implemented (Build 22) |
| `/shop/product_list` | `ProductListRequest` | `ProductListResponse` | implemented |
| `/shop/shop_buy` | `ShopBuyRequest` | `ShopBuyResponse` | logged as unknown |
| `/shop/shop_list` | `ShopListRequest` | `ShopListResponse` | implemented |

### gasha — gacha (5, 0 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/gasha/gasha_detail` | `GashaDetailRequest` | `GashaDetailResponse` (M) | logged as unknown |
| `/gasha/gasha_list` | `GashaListRequest` | `GashaListResponse` | logged as unknown |
| `/gasha/gasha_play` | `GashPlayRequest` | `GashaPlayResponse` | logged as unknown |
| `/gasha/get_box_gasha` | `GetBoxGashaRequest` | `GetBoxGashaResponse` | logged as unknown |
| `/gasha/reset_box_gasha` | `ResetBoxGashaRequest` | `ResetBoxGashaResponse` | logged as unknown |

### present — present box (3, 1 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/present/get` | `GetPresentRequest` | `GetPresentResponse` | logged as unknown |
| `/present/get_all` | `GetPresentAllRequest` | `GetPresentResponse` | logged as unknown |
| `/present/present_list` | `PresentListRequest` | `PresentListResponse` | implemented |

### team — team formation (11, 1 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/team/auto_set_and_equip` | `AutoFomationRequest` | `AutoFomationResponse` | logged as unknown |
| `/team/equip_mind` | `EquipMindRequest` | `EquipMindResponse` | logged as unknown |
| `/team/equip_multiple_assist` | `EquipMultipleAssistRequest` | `EquipMultipleAssistResponse` | logged as unknown |
| `/team/equip_multiple_minds` | `EquipMultipleMindsRequest` | `EquipMultipleMindsResponse` | logged as unknown |
| `/team/leader_change` | `LeaderChangeRequest` | `LeaderChangeResponse` | logged as unknown |
| `/team/save_all_flg` | `SaveAllFlgRequest` | `SaveAllFlgResponse` | logged as unknown |
| `/team/set_team` | `SetTeamRequest` | `SetTeamResponse` | logged as unknown |
| `/team/set_team_all` | `SetTeamAllRequest` | `SetTeamAllResponse` | logged as unknown |
| `/team/statue_select` | `StatueChangeRequest` | `StatueChangeResponse` | logged as unknown |
| `/team/team_info` | `TeamInfoRequest` | `TeamInfoResponse` | implemented |
| `/team/unlock_mind_seq` | `UnlockMindRequest` | `UnlockMindResponse` | logged as unknown |

### statue — characters (statues) (10, 0 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/statue/add_storage` | `StatueAddStorageRequest` | `StatueAddStorageResponse` (M) | logged as unknown |
| `/statue/evolution_exec` | `StatueEvolutionExecRequest` | `StatueEvolutionExecResponse` | logged as unknown |
| `/statue/evolution_info` | `StatueEvolutionInfoRequest` | `StatueEvolutionInfoResponse` | logged as unknown |
| `/statue/new_flg_release` | `StatueNewFlgReleaseRequest` | `StatueNewFlgReleaseResponse` | logged as unknown |
| `/statue/protect` | `StatueProtectRequest` | `StatueProtectResponse` | logged as unknown |
| `/statue/save_sort` | `StatueSaveSortRequest` | `StatueSaveSortResponse` | logged as unknown |
| `/statue/statue_info` | `StatueInfoRequest` | `StatueInfoResponse` | implemented |
| `/statue/statue_list` | `StatueListRequest` | `StatueListResponse` | implemented |
| `/statue/strengthening` | `StatueStrengtheningRequest` | `StatueStrengtheningResponse` | logged as unknown |
| `/statue/strengthening_item` | `StatueStrengtheningItemRequest` | `StatueStrengtheningResponse` | logged as unknown |

### mind — equipment (minds) (11, 0 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/mind/add_storage` | `MindAddStorageRequest` | `MindAddStorageResponse` (M) | logged as unknown |
| `/mind/mind_info` | `MindInfoRequest` | `MindInfoResponse` | implemented |
| `/mind/mind_list` | `MindListRequest` | `MindListResponse` | implemented |
| `/mind/mind_storage_list` | `MindStorageListRequest` | `MindStorageListResponse` | logged as unknown |
| `/mind/mind_storage_move` | `MindStorageMoveRequest` | `MindStorageMoveResponse` | logged as unknown |
| `/mind/new_flg_release` | `MindNewFlgReleaseRequest` | `MindNewFlgReleaseResponse` | logged as unknown |
| `/mind/protect` | `MindProtectRequest` | `MindProtectResponse` | logged as unknown |
| `/mind/protect_multi` | `MindProtectMultiRequest` | `MindProtectMultiResponse` | logged as unknown |
| `/mind/save_sort` | `MindSaveSortRequest` | `MindSaveSortResponse` | logged as unknown |
| `/mind/strengthening` | `MindStrengtheningRequest` | `MindStrengtheningResponse` | logged as unknown |
| `/mind/strengthening_item` | `MindStrengtheningItemRequest` | `MindStrengtheningResponse` | logged as unknown |

### item — items (3, 1 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/item/exp_item_list` | `ExpItemListRequest` | `ExpItemListResponse` | logged as unknown |
| `/item/heal_item_list` | `HealItemListRequest` | `HealItemListResponse` | logged as unknown |
| `/item/item_list` | `ItemListRequest` | `ItemListResponse` | implemented |

### sell — selling (1, 0 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/sell/sell` | `SellRequest` | `SellResponse` | logged as unknown |

### tree — skill tree (2, 0 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/tree/acquisition` | `TreeSquareRequest` | `TreeSquareResponse` | logged as unknown |
| `/tree/skill_list` | `TreeListRequest` | `TreeListResponse` | logged as unknown |

### trophy — missions/trophies (3, 0 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/trophy/get_all_trophy_result` | `GetAllTrophyResult` | `GetAllTrophyResponse` | logged as unknown |
| `/trophy/trophy_list` | `TrophyListRequest` | `TrophyListResponse` | logged as unknown |
| `/trophy/trophy_result_get` | `TrophyResultGetRequest` | `TrophyResultGetResponse` | logged as unknown |

### book — collection book (4, 0 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/book/item_list` | `ItemBookRequest` | `ItemBookResponse` | logged as unknown |
| `/book/mind_list` | `MindBookRequest` | `MindBookResponse` | logged as unknown |
| `/book/statue_list` | `StatueBookRequest` | `StatueBookResponse` | logged as unknown |
| `/book/yogo_list` | `TermBookRequest` | `TermBookResponse` (M) | logged as unknown |

### bizarre — story/quest battles (10, 2 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/bizarre/bizarre_continue` | `BizarreContinueRequest` | `BizarreContinueResponse` | implemented |
| `/bizarre/bizarre_end` | `BizarreEndRequest` | `BizarreEndResponse` | implemented |
| `/bizarre/bizarre_list` | `BizarreListRequest` | `BizarreListResponse` | implemented |
| `/bizarre/bizarre_resumption` | `BizarreResumptionRequest` | `BizarreResumptionResponse` | implemented |
| `/bizarre/bizarre_start` | `BizarreStartRequest` | `BizarreStartResponse` | implemented |
| `/bizarre/check_timed_chapter` | `CheckTimedChapterRequest` | `CheckTimedChapterResponse` (M) | implemented |
| `/bizarre/destruction` | `BizarreDestructionRequest` | `BizarreDestructionResponse` | implemented |
| `/bizarre/get_bizarre_select` | `BizarreSelectRequest` | `BizarreSelectResponse` | implemented |
| `/bizarre/stage_list` | `StageListRequest` | `StageListResponse` | implemented |
| `/bizarre/unlock` | `BizarreUnlockRequest` | `BizarreUnlockResponse` | logged as unknown |

### dungeon — tarot dungeon (6, 0 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/dungeon/dungeon_list` | `DungeonListRequest` | `DungeonListResponse` | logged as unknown |
| `/dungeon/earn_reward` | `DungeonEarnRewardRequest` | `DungeonEarnRewardResponse` | logged as unknown |
| `/dungeon/reward_check` | `DungeonRewardCheckRequest` | `DungeonRewardCheckResponse` | logged as unknown |
| `/dungeon/stage_list` | `DungeonStageListRequest` | `DungeonStageListResponse` | logged as unknown |
| `/dungeon/tarot_flip` | `DungeonTarotFlipRequest` | `DungeonTarotFlipResponse` | logged as unknown |
| `/dungeon/tarot_select` | `DungeonTarotSelectRequest` | `DungeonTarotSelectResponse` | logged as unknown |

### raid — raid battles (6, 6 implemented: an inactive raid, docs/research/25-events-raids.md §1)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/raid/get_raid_reward` | `GetRaidRewardRequest` | `GetRaidRewardResponse` | implemented (Build 25; no caller in 3.12) |
| `/raid/raid_end` | `RaidEndRequest` | `RaidEndResponse` | implemented (Build 25; a withdrawal) |
| `/raid/raid_history_list` | `RaidHistoryListRequest` | `RaidHistoryListResponse` | implemented (Build 25; no caller in 3.12) |
| `/raid/raid_list` | `RaidListRequest` | `RaidListResponse` | implemented (Build 25; no caller in 3.12) |
| `/raid/raid_offer_rescue` | `RaidOfferRescueRequest` | `RaidOfferRescueResponse` | implemented (Build 25; fails) |
| `/raid/raid_start` | `RaidStartRequest` | `RaidStartResponse` | implemented (Build 25; refused, timed out) |

### pvp — PvP (17, 17 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/pvp/auto_set_and_equip` | `PvpAutoFomationRequest` | `PvpAutoFomationResponse` | implemented (Build 23, research/23) |
| `/pvp/enemy_player_list` | `PvpOpponentInfoRequest` | `PvpOpponentInfoResponse` | implemented (Build 23, research/23) |
| `/pvp/equip_mind_pvp` | `PvpEquipMindRequest` | `PvpEquipMindResponse` | implemented (Build 23, research/23) |
| `/pvp/equip_multiple_assist_pvp` | `PvpEquipMultipleAssistRequest` | `PvpEquipMultipleAssistResponse` | implemented (Build 23, research/23) |
| `/pvp/equip_multiple_minds_pvp` | `PvpEquipMultipleMindsRequest` | `PvpEquipMultipleMindsResponse` | implemented (Build 23, research/23) |
| `/pvp/fever_mode` | `PvpFeverModeStartRequest` | `PvpFeverModeStartResponse` | implemented (Build 23, research/23) |
| `/pvp/get_pvp_config` | `Pvp2GetConfigRequest` | `Pvp2GetConfigResponse` | implemented (Build 23, research/23) |
| `/pvp/home` | `PvpHomeInfoRequest` | `PvpHomeInfoResponse` | implemented (Build 23, research/23) |
| `/pvp/home_acqusition_check` | `PvpHomeAcqusitionCheckRequest` | `PvpHomeAcqusitionCheckResponse` | implemented (Build 23, research/23) |
| `/pvp/leader_change_pvp` | `PvpLeaderChangeRequest` | `PvpLeaderChangeResponse` | implemented (Build 23, research/23) |
| `/pvp/pvp_end` | `PvpBattleEndRequest` | `PvpBattleEndResponse` | implemented (Build 23, research/23) |
| `/pvp/pvp_start` | `PvpBattleStartRequest` | `PvpBattleStartResponse` | implemented (Build 23, research/23) |
| `/pvp/recover_hp_pvp` | `Pvp2RecoveryHpRequest` | `Pvp2RecoveryHpResponse` | implemented (Build 23, research/23) |
| `/pvp/set_team_all_pvp` | `PvpSetTeamAllRequest` | `PvpSetTeamAllResponse` | implemented (Build 23, research/23) |
| `/pvp/set_team_pvp` | `PvpSetTeamRequest` | `PvpSetTeamResponse` (M) | implemented (Build 23, research/23) |
| `/pvp/statue_select_pvp` | `PvpStatueChangeRequest` | `PvpStatueChangeResponse` | implemented (Build 23, research/23) |
| `/pvp/team_info_pvp` | `PvpTeamInfoPvpRequest` | `PvpTeamInfoPvpResponse` | implemented (Build 23, research/23) |

### event — event points and rankings (5, 5 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/event/clan_member_event_point_list` | `EventClanMemberPointListRequest` | `EventClanMemberPointListResponse` | implemented (Build 25, research/24) |
| `/event/get_event_ranking_detail` | `RankingDetailRequest` | `RankingDetailResponse` | implemented (Build 24, research/23 §3) |
| `/event/get_event_ranking_list` | `RankingListRequest` | `RankingListResponse` | implemented (Build 24) |
| `/event/get_event_ranking_reward` | `RankingRewardRequest` | `RankingRewardResponse` | implemented (Build 24) |
| `/event/point_reward_list` | `EventPointListRequest` | `EventPointListResponse` | implemented (Build 23) |

### support — support (helper) players (1, 0 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/support/support_user_s` | `SupportUserRequest` | `SupportUserResponse` | implemented |

### follow — follow list (social) (10, 1 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/follow/block` | `BlockUserRequest` | `BlockUserResponse` | logged as unknown |
| `/follow/block_list` | `BlockListRequest` | `BlockListResponse` (M) | logged as unknown |
| `/follow/follow_list` | `FollowListRequest` | `FollowListResponse` (M) | implemented |
| `/follow/follower_list` | `FollowerListRequest` | `FollowerListResponse` (M) | logged as unknown |
| `/follow/kick_follow` | `ReleaseFollowerRequest` | `ReleaseFollowerResponse` (M) | logged as unknown |
| `/follow/release_follow` | `ReleaseFollowRequest` | `SetFollowResponse` | logged as unknown |
| `/follow/save_sort` | `SaveSortFollowRequest` | `SaveSortFollowResponse` (M) | logged as unknown |
| `/follow/search` | `SearchUserRequest` | `SearchUserResponse` (M) | logged as unknown |
| `/follow/set_follow` | `SetFollowRequest` | `SetFollowResponse` | logged as unknown |
| `/follow/unblock` | `UnblockUserRequest` | `UnblockUserResponse` | logged as unknown |

### clan — the Town, offline with AI members (27, 27 implemented)

| Path | Request class | Response class | Build 01 |
|---|---|---|---|
| `/clan/add` | `AddClanRequest` | `AddClanResponse` | implemented (Build 25, research/24) |
| `/clan/add_approval` | `AddApprovalClanRequest` | `AddApprovalClanResponse` | implemented (Build 25, research/24) |
| `/clan/add_request_list` | `AddListClanRequest` | `AddListClanResponse` | implemented (Build 25, research/24) |
| `/clan/all_del_object` | `AllRemoveHasObjectRequest` | `AllRemoveHasObjectResponse` (M) | implemented (Build 25: no caller in 3.12.0, empty `objList`) |
| `/clan/cancel_approval` | `CancelAddClanRequest` | `CancelAddClanResponse` | implemented (Build 25, research/24) |
| `/clan/change_info` | `ChangeClanInfoRequest` | `ChangeClanInfoResponse` | implemented (Build 25, research/24) |
| `/clan/check_add_request` | `CheckAddRequest` | `CheckAddResponse` | implemented (Build 25, research/24) |
| `/clan/clan_bg_list` | `ClanBgListRequest` | `ClanBgListResponse` | implemented (Build 25, research/24) |
| `/clan/clan_donation` | `DonationRequest` | `DonationResponse` | implemented (Build 25, research/24) |
| `/clan/clan_object_list` | `ClanObjectListRequest` | `ClanObjectListResponse` | implemented (Build 25, research/24) |
| `/clan/create` | `CreateClanRequest` | `CreateClanResponse` | implemented (Build 25, research/24) |
| `/clan/del_object` | `RemoveClanObjectRequest` | `RemoveClanObjectResponse` (M) | implemented (Build 25: no caller in 3.12.0, empty `objList`) |
| `/clan/denial_approval` | `DenialApprovalClanRequest` | `DenialApprovalClanResponse` | implemented (Build 25, research/24) |
| `/clan/donation_item_list` | `DonationItemListRequest` | `DonationItemListResponse` | implemented (Build 25, research/24) |
| `/clan/expulsion` | `ExplusionClanRequest` | `ExplusionClanResponse` | implemented (Build 25, research/24) |
| `/clan/info` | `GetClanInfoRequest` | `GetClanInfoResponse` | implemented (Build 25, research/24) |
| `/clan/invitation` | `InviteClanRequest` | `InviteClanResponse` | implemented (Build 25, research/24) |
| `/clan/recommend` | `GetRecommendClanRequest` | `GetRecommendClanResponse` | implemented (Build 25, research/24) |
| `/clan/room_info` | `GetClanRoomInfoRequest` | `GetClanRoomInfoResponse` (M) | implemented (Build 25: no caller in 3.12.0, empty `objList`) |
| `/clan/search` | `SearchIDClanRequest` | `SearchClanResponse` | implemented (Build 25, research/24) |
| `/clan/search` | `SearchNameClanRequest` | `SearchClanResponse` | implemented (Build 25, research/24) |
| `/clan/search` | `SearchPolicyClanRequest` | `SearchClanResponse` | implemented (Build 25, research/24) |
| `/clan/secession` | `LeaveClanRequest` | `LeaveClanResponse` | implemented (Build 25, research/24) |
| `/clan/set_object` | `SetClanObjectRequest` | `SetClanObjectResponse` (M) | implemented (Build 25: no caller in 3.12.0, empty `objList`) |
| `/clan/set_object_comment` | `SetCommentClanObjectRequest` | `SetCommentClanObjectResponse` (M) | implemented (Build 25: no caller in 3.12.0, empty `objList`) |
| `/clan/set_sub_leader` | `SetSubMasterRequest` | `SetSubMasterResponse` | implemented (Build 25, research/24) |
| `/clan/sync_server_info` | `GetConnectServerInfoRequest` | `GetConnectServerInfoResponse` | implemented (Build 25, research/24) |
