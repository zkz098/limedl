---
id: systems/persistence-and-recovery
title: SQLite Persistence, Durable Progress and Crash Recovery
status: verified
generated_at: 2026-11-13T03:30:34Z
commit: b56ef11879fd182cdb250f518a7360caf8eda9c3
evidence:
- path: crates/limedl-engine/src/context.rs
  lines:
  - 58
  - 59
  blob: 26aa8d3d8e5c1669ccede7ef4f1c3f9670669554
  snippet_sha1: 6c447bbc74e0a55ea44f8632c3bf607cdc802c94
  note: rename-detected crates/limedl-core/src/context.rs -> crates/limedl-engine/src/context.rs (identical blob)
- path: crates/limedl-store/src/lib.rs
  lines:
  - 1
  - 21
  blob: 79a57011d6856f5ebd05f2a9429c9e75f591f7f0
  snippet_sha1: d06fb243d780d41383c31fa80e7e3a9da6a99062
- path: crates/limedl-engine/src/persistence.rs
  lines:
  - 1
  - 21
  blob: dac6df18b1fbb4ac265f3454ec8096905dbe3057
  snippet_sha1: ad87bdff55895bb4e7e0127e439a75e66cd98a18
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 1110
  - 1124
  blob: 99d0894f5aa328b489e88b43c5664cab0759887a
  snippet_sha1: 56403cfe6fa0b94ee46badd089797fa099001728
  note: drift-projected L1101-L1115@2361c059 -> L1110-L1124 (similarity 1.00)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 17
  - 21
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: dc235cbd97e1ce52707ce4182907951170d6cac5
  note: drift-projected L17-L20@2c6501bf -> L17-L21 (similarity 1.00)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 28
  - 62
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: 4f8493d2beff8cb9ad2636bf77d522994cba4070
  note: drift-projected L27-L61@2c6501bf -> L28-L62 (similarity 0.97)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 141
  - 152
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: bfc3aeb88f3e0d46bd16045e7d0e65df969c9a33
  note: drift-projected L140-L151@2c6501bf -> L141-L152 (similarity 1.00)
- path: crates/limedl-engine/src/persistence.rs
  lines:
  - 77
  - 89
  blob: dac6df18b1fbb4ac265f3454ec8096905dbe3057
  snippet_sha1: a072a04bd1ac2097d5279b0d7c7eeb866f03411f
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 1163
  - 1167
  blob: 99d0894f5aa328b489e88b43c5664cab0759887a
  snippet_sha1: 5e76ec0cc51c3c8c8b8e8c4cf3eb140d4bb6c01f
  note: drift-projected L1154-L1158@2361c059 -> L1163-L1167 (similarity 1.00)
- path: crates/limedl-store/src/database/chunk_repo.rs
  lines:
  - 73
  - 90
  blob: 0bd58c6ca0db07244f40d8a66471f0c56c5ae265
  snippet_sha1: 1181c88bde8d985da3b75df42805d7a8966ef8c2
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 12
  - 15
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: c159a49289b5b6c11846d08180b098991913667c
  note: drift-projected L12-L15@2c6501bf -> L12-L15 (similarity 1.00)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 344
  - 362
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: ebe124c86734105666c11cd5bd762267fcc0e519
  note: drift-projected L319-L336@2c6501bf -> L344-L362 (similarity 0.78)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 386
  - 406
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: d0b2172dfc4fd925ea21891bdfc7bf0f7f91ed72
  note: drift-projected L360-L381@2c6501bf -> L386-L406 (similarity 0.86)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 308
  - 319
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: 0d8f7c119935b95a1f8ec5c15be661c14978dee2
  note: drift-projected L282-L293@2c6501bf -> L308-L319 (similarity 1.00)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 301
  - 306
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: 723a087b29afd5efca2e73f3cff474d344648ed3
  note: drift-projected L275-L280@2c6501bf -> L301-L306 (similarity 1.00)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 321
  - 336
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: 8d6a1a1d99b80b5bffa234d5677cf4a9351b2e32
  note: drift-projected L295-L310@2c6501bf -> L321-L336 (similarity 1.00)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 364
  - 384
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: f16b7903cbb9f10d604872d07025459780e924bd
  note: drift-projected L338-L358@2c6501bf -> L364-L384 (similarity 0.95)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 260
  - 299
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: ee06048e4d96991b43fb690b02e9cafac0495fe0
  note: drift-projected L234-L273@2c6501bf -> L260-L299 (similarity 1.00)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 338
  - 344
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: db3570f847cba277343a603fb0b43532d934151c
  note: drift-projected L312-L318@2c6501bf -> L338-L344 (similarity 1.00)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 64
  - 67
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: 3f36cd06c670f8fd572eae5adc97ac83d116f412
  note: drift-projected L63-L66@2c6501bf -> L64-L67 (similarity 1.00)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 92
  - 139
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: 571278ab650160cc7caae397588190a61df75acb
  note: drift-projected L91-L138@2c6501bf -> L92-L139 (similarity 1.00)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 99
  - 130
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: 6f56b938e74143693b137675fd6bbfe3630fc6b4
  note: drift-projected L98-L129@2c6501bf -> L99-L130 (similarity 1.00)
- path: crates/limedl-store/src/database/schema.rs
  lines:
  - 21
  - 49
  blob: ea561ba2f671bb2bd40c2a609e106f177aa5e9b4
  snippet_sha1: 9d32362d1965c98f9833cc38f668d5af9c13ce1e
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 69
  - 90
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: 6d281f28f4d6a3454cedce571ecf3192da186110
  note: drift-projected L68-L89@2c6501bf -> L69-L90 (similarity 1.00)
- path: crates/limedl-store/src/database/schema.rs
  lines:
  - 4
  - 19
  blob: ea561ba2f671bb2bd40c2a609e106f177aa5e9b4
  snippet_sha1: da9111d7598784f6be04e2ce696badac867aa75a
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 161
  - 191
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: fa81900a740d2a1fe95a8734fcd1109f287edfd1
  note: drift-projected L159-L188@2c6501bf -> L161-L191 (similarity 1.00)
- path: crates/limedl-store/src/database/schema.rs
  lines:
  - 51
  - 107
  blob: ea561ba2f671bb2bd40c2a609e106f177aa5e9b4
  snippet_sha1: 1aca8593f03a4d1d40630a1f3f670514d61dc16c
- path: crates/limedl-store/src/database/schema.rs
  lines:
  - 115
  - 258
  blob: ea561ba2f671bb2bd40c2a609e106f177aa5e9b4
  snippet_sha1: 6a68edd22f6c029f2d644802365803a7ea12ece2
- path: crates/limedl-store/src/database/manifest_repo.rs
  lines:
  - 16
  - 119
  blob: a036d5150ef109be13825a764ff421b3e03f6e42
  snippet_sha1: 9096fbdebc50996c5eed7ae8ef600d6a0b716cf3
  note: drift-projected L16-L119@92bf2011 -> L16-L119 (similarity 1.00)
- path: crates/limedl-store/src/database/manifest_repo.rs
  lines:
  - 126
  - 128
  blob: a036d5150ef109be13825a764ff421b3e03f6e42
  snippet_sha1: cea5b672268407ccc36d02d22f87d8fe6ac27405
  note: drift-projected L126-L128@92bf2011 -> L126-L128 (similarity 1.00)
- path: crates/limedl-store/src/database/bt_task_repo.rs
  lines:
  - 36
  - 45
  blob: 92d808708cf01c14b9ff597c3f60949826d43430
  snippet_sha1: c0ccd70ce2c0097b3bccb63a6d772761536d0e91
  note: drift-projected L36-L45@184112a7 -> L36-L45 (similarity 1.00)
- path: crates/limedl-types/src/manifest.rs
  lines:
  - 201
  - 205
  blob: 6b6398443471f60ea175d08c3f3feb508d636146
  snippet_sha1: bd12f3df22b7757dea70e06169a05a0f565e3fa0
  note: drift-projected L201-L205@54ad6848 -> L201-L205 (similarity 1.00)
- path: crates/limedl-store/src/database/manifest_repo.rs
  lines:
  - 77
  - 99
  blob: a036d5150ef109be13825a764ff421b3e03f6e42
  snippet_sha1: ec4ff157b353ef1bae328983eade09cb7813a341
  note: drift-projected L77-L99@92bf2011 -> L77-L99 (similarity 1.00)
- path: crates/limedl-store/src/database/manifest_repo.rs
  lines:
  - 126
  - 194
  blob: a036d5150ef109be13825a764ff421b3e03f6e42
  snippet_sha1: 77e32755476ad12b8c53a584fc2b3df22d4d755a
  note: drift-projected L126-L194@92bf2011 -> L126-L194 (similarity 1.00)
- path: crates/limedl-store/src/database/manifest_repo.rs
  lines:
  - 330
  - 417
  blob: a036d5150ef109be13825a764ff421b3e03f6e42
  snippet_sha1: 7556246e3913e9ac89e29939c694dedae6f52245
  note: drift-projected L330-L394@92bf2011 -> L330-L417 (similarity 0.94)
- path: crates/limedl-types/src/types/common.rs
  lines:
  - 15
  - 27
  blob: 22d9bc8410a3945fcd353adba848b94dd013180b
  snippet_sha1: aa9c900b8d9fa2ea1a884bfa6e5c1f48ecf7d3b3
- path: crates/limedl-types/src/checksum/mod.rs
  lines:
  - 18
  - 41
  blob: 2e7e0d9ae916d869ccaf128453542a39b74d0c0e
  snippet_sha1: c36b626a6609c434b94d352d68bbe908e9516e7b
- path: crates/limedl-types/src/checksum/mod.rs
  lines:
  - 63
  - 100
  blob: 2e7e0d9ae916d869ccaf128453542a39b74d0c0e
  snippet_sha1: 25ac353ed3cee29747449899754c0690b4ad4468
- path: crates/limedl-types/src/types/common.rs
  lines:
  - 21
  - 29
  blob: 22d9bc8410a3945fcd353adba848b94dd013180b
  snippet_sha1: 013c2a081bf902466efb4b0f55850bc0b480ae1f
- path: crates/limedl-types/src/types/settings.rs
  lines:
  - 360
  - 379
  blob: d36d3f34aad95b687828c40c0c27eaae53fccddb
  snippet_sha1: 6aecedf4c9cceee499a1e641b3107e86aebf935f
- path: crates/limedl-types/src/checksum/mod.rs
  lines:
  - 18
  - 100
  blob: 2e7e0d9ae916d869ccaf128453542a39b74d0c0e
  snippet_sha1: 16923ef8001ba9784a79c3f656f25b32cb6ea6df
- path: crates/limedl-store/src/database/manifest_repo.rs
  lines:
  - 68
  - 99
  blob: a036d5150ef109be13825a764ff421b3e03f6e42
  snippet_sha1: 7752248fd8bf98ca63cbe47fdc0bba13a4275ce2
  note: drift-projected L68-L99@92bf2011 -> L68-L99 (similarity 1.00)
- path: crates/limedl-rpc/src/options.rs
  lines:
  - 431
  - 463
  blob: 3bf801ef8406717e6ada43b881f3036601097eb8
  snippet_sha1: 58b30ed4cabc78f1be742edee9e8a6d8b3fe8fa9
- path: crates/limedl-native/src/bridge/forms/enums.rs
  lines:
  - 73
  - 86
  blob: 23e2b4df2fe65e0734fce0496db8835ae305dc2f
  snippet_sha1: 65f9bc292209373ac21817a4f85852764f6784f0
- path: crates/limedl-native/src/bridge/forms/combo.rs
  lines:
  - 11
  - 11
  blob: 30046b35548db5d380cdd08cbb1a22fb5e81fe5c
  snippet_sha1: 9b47065be84b9cf9502662069765f54c0de16666
- path: crates/limedl-native/ui/types.slint
  lines:
  - 187
  - 187
  blob: da4c0080fe336bc1ff5d19c55af480fc2bde2258
  snippet_sha1: ad7ad0fe5fd0c4ed7a362d2285f7c3f6244d59e4
- path: crates/limedl-native/ui/components/settings/tab_download.slint
  lines:
  - 73
  - 79
  blob: e89247504e1a556c92c6a796cbbb2c0d1cbee0a5
  snippet_sha1: f23b9f2f0ee999d6d1b96f3300bbbc91f87574f4
- path: crates/limedl-native/lang/en/LC_MESSAGES/limedl-native.po
  lines:
  - 917
  - 924
  blob: 0012f335bc581a08bb3b0bf8f60f01a76834526b
  snippet_sha1: 0ae6fdde940b8a786dbbd8e4085c34e33dc0909e
  note: drift-projected L917-L924@d7a4a2f3 -> L917-L924 (similarity 1.00)
- path: crates/limedl-native/lang/zh_CN/LC_MESSAGES/limedl-native.po
  lines:
  - 917
  - 926
  blob: 65abc80b400eaf53b4ee4b92d5b19b35726acb7d
  snippet_sha1: f92c2e88e3f9b997d4dad7cb611e458e92b49f6f
  note: drift-projected L917-L926@ad7123ed -> L917-L926 (similarity 1.00)
- path: crates/limedl-types/src/settings/mod.rs
  lines:
  - 522
  - 545
  blob: e8fe36a862b028e422dacb74d56b0e135d9c84bf
  snippet_sha1: 24b6c9540a21553230cce34dfbcc1147c0aad9a7
- path: crates/limedl-types/src/manifest.rs
  lines:
  - 75
  - 95
  blob: 6b6398443471f60ea175d08c3f3feb508d636146
  snippet_sha1: 5760b01d7756332d5237d1788102945cb5205d71
  note: drift-projected L75-L95@54ad6848 -> L75-L95 (similarity 1.00)
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 24
  - 41
  blob: 4adcb7ff251dfe41230bee5d56018e575c67bbe9
  snippet_sha1: cfb0955a88c0a04d69b86501fd78b4aceb0dc225
  note: drift-projected L20-L32@1425cef6 -> L24-L41 (similarity 0.92)
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 114
  - 138
  blob: 4adcb7ff251dfe41230bee5d56018e575c67bbe9
  snippet_sha1: 7f091e284edb66fc99e318a8eb537e63eaf242cb
  note: drift-projected L155-L182@1425cef6 -> L114-L138 (similarity 0.89)
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 140
  - 176
  blob: 4adcb7ff251dfe41230bee5d56018e575c67bbe9
  snippet_sha1: 89c1608e81ef428039cfa44a0f6ccaee4054e1eb
  note: drift-projected L184-L220@1425cef6 -> L140-L176 (similarity 1.00)
- path: crates/limedl-store/src/database/chunk_repo.rs
  lines:
  - 27
  - 46
  blob: 0bd58c6ca0db07244f40d8a66471f0c56c5ae265
  snippet_sha1: 4262eae4f5d28c5348dc6493aebcac40c5eff851
- path: crates/limedl-types/src/manifest.rs
  lines:
  - 110
  - 124
  blob: 6b6398443471f60ea175d08c3f3feb508d636146
  snippet_sha1: 042beda36e2d12a19c55191d036270b9c1b8ee44
  note: drift-projected L110-L124@54ad6848 -> L110-L124 (similarity 1.00)
- path: crates/limedl-engine/src/download/shared.rs
  lines:
  - 16
  - 18
  blob: 4fd41e05b1cb50e99f9aad5e048f9a31ab2fba7f
  snippet_sha1: 7ae364bdb860356bdbfc13e30f5396b24780e9cf
- path: crates/limedl-engine/src/persistence.rs
  lines:
  - 23
  - 71
  blob: dac6df18b1fbb4ac265f3454ec8096905dbe3057
  snippet_sha1: c89e17b5acce05fdf81cff7bb3d0dfc4e6cc868e
- path: crates/limedl-engine/src/persistence.rs
  lines:
  - 113
  - 155
  blob: dac6df18b1fbb4ac265f3454ec8096905dbe3057
  snippet_sha1: 32c1ebe7d16347532f53306017ed21828f719325
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 1172
  - 1185
  blob: 99d0894f5aa328b489e88b43c5664cab0759887a
  snippet_sha1: 0585c591d9a7fb641781404902358cb5b15f416d
  note: drift-projected L1163-L1176@2361c059 -> L1172-L1185 (similarity 1.00)
- path: crates/limedl-store/src/database/chunk_repo.rs
  lines:
  - 63
  - 90
  blob: 0bd58c6ca0db07244f40d8a66471f0c56c5ae265
  snippet_sha1: 4c987ed0458b31fc2b1bc8bfb224ff78479b74b0
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 140
  - 152
  blob: 4adcb7ff251dfe41230bee5d56018e575c67bbe9
  snippet_sha1: 76aedc5be8fa74ab164a029ea71dee04373dfb48
  note: drift-projected L184-L196@1425cef6 -> L140-L152 (similarity 1.00)
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 1091
  - 1170
  blob: 99d0894f5aa328b489e88b43c5664cab0759887a
  snippet_sha1: 1668cf16098e7b33d6c6dfb0ea2c9ae71aedfe58
  note: drift-projected L1082-L1161@2361c059 -> L1091-L1170 (similarity 1.00)
- path: crates/limedl-store/src/database/chunk_repo.rs
  lines:
  - 48
  - 61
  blob: 0bd58c6ca0db07244f40d8a66471f0c56c5ae265
  snippet_sha1: b72c4481dc9f41971f3f84fabbc2b969d2e11caa
- path: crates/limedl-engine/src/download/managed.rs
  lines:
  - 42
  - 53
  blob: 4adcb7ff251dfe41230bee5d56018e575c67bbe9
  snippet_sha1: c5f6d6d95c3723a45256023d8b51254254ea9978
  note: drift-projected L33-L42@1425cef6 -> L42-L53 (similarity 0.80)
- path: crates/limedl-engine/src/http_executor/worker.rs
  lines:
  - 191
  - 211
  blob: 717e6ee1277636bb8979862472d1403ef1eeff40
  snippet_sha1: 63652c87980b4960e8988f2a6fbc563871e33372
  note: drift-projected L190-L210@786277ec -> L191-L211 (similarity 1.00)
- path: crates/limedl-store/src/database/bt_task_repo.rs
  lines:
  - 20
  - 60
  blob: 92d808708cf01c14b9ff597c3f60949826d43430
  snippet_sha1: 78f0653b263be275fb91976ab505968f33dbf6ab
  note: drift-projected L20-L60@184112a7 -> L20-L60 (similarity 1.00)
- path: crates/limedl-store/src/database/bt_task_repo.rs
  lines:
  - 62
  - 97
  blob: 92d808708cf01c14b9ff597c3f60949826d43430
  snippet_sha1: 5afb5d5f574ba490d99c4a0033de02091a7f1a3e
  note: drift-projected L62-L97@184112a7 -> L62-L97 (similarity 1.00)
- path: crates/limedl-store/src/database/bt_task_repo.rs
  lines:
  - 1
  - 12
  blob: 92d808708cf01c14b9ff597c3f60949826d43430
  snippet_sha1: 3ac7142fdffae24c0697de7fc69589286d03fd52
  note: drift-projected L1-L12@184112a7 -> L1-L12 (similarity 1.00)
- path: crates/limedl-bt/src/lazy.rs
  lines:
  - 674
  - 685
  blob: 7b6185c3e58c5da27f87e499637f4a37fd9acf57
  snippet_sha1: 8e8e3f8912e774f5bc631f2b45840f9aa8f46f4b
  note: drift-projected L674-L690@e73993a3 -> L674-L685 (similarity 0.65)
- path: crates/limedl-bt/src/lazy.rs
  lines:
  - 715
  - 721
  blob: 7b6185c3e58c5da27f87e499637f4a37fd9acf57
  snippet_sha1: 9d3e802cd58cba7cceeb50375fef31584485a01a
  note: content lost at HEAD; re-pinned at recorded range (draft)
- path: crates/limedl-bt/src/lazy.rs
  lines:
  - 313
  - 329
  blob: 7b6185c3e58c5da27f87e499637f4a37fd9acf57
  snippet_sha1: 55b901fe6961bef299c979c92872f020ebc68e2d
  note: drift-projected L313-L329@e73993a3 -> L313-L329 (similarity 1.00)
- path: crates/limedl-store/src/database/manifest_repo.rs
  lines:
  - 428
  - 456
  blob: a036d5150ef109be13825a764ff421b3e03f6e42
  snippet_sha1: 32485e5b087de2abb7a42df6641e134143bf089c
  note: drift-projected L405-L433@92bf2011 -> L428-L456 (similarity 1.00)
- path: crates/limedl-store/src/database/manifest_repo.rs
  lines:
  - 459
  - 468
  blob: a036d5150ef109be13825a764ff421b3e03f6e42
  snippet_sha1: c875244c68d3065c592f1796359781536eda4de6
  note: drift-projected L436-L445@92bf2011 -> L459-L468 (similarity 1.00)
- path: crates/limedl-store/src/database/manifest_repo.rs
  lines:
  - 538
  - 553
  blob: a036d5150ef109be13825a764ff421b3e03f6e42
  snippet_sha1: a6bfeccd9a5cf62193d073f51ded1922511d00fc
  note: drift-projected L515-L530@92bf2011 -> L538-L553 (similarity 1.00)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 205
  - 221
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: 0bf516ff565d5b14baffab60833feadc384fc9d5
  note: drift-projected L205-L224@b8cd589f -> L205-L221 (similarity 0.80)
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 462
  - 474
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: 6274f603bbadb0be81412963fa52531f0e41f259
  note: drift-projected L468-L480@6d4ba265 -> L462-L474 (similarity 1.00)
- path: crates/limedl-store/src/database/connection.rs
  lines:
  - 207
  - 257
  blob: 8abbc9a50d2ca790bc7fa1ba706464f72d2a98a2
  snippet_sha1: 97acaa42ae7422a21d55a0ade08aa9d3c10ac356
  note: drift-projected L198-L231@2c6501bf -> L207-L257 (similarity 1.00)
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 1081
  - 1085
  blob: 99d0894f5aa328b489e88b43c5664cab0759887a
  snippet_sha1: 1798cafd48d4b45c82f335aa3cf5468fa7b8b870
  note: drift-projected L1072-L1076@2361c059 -> L1081-L1085 (similarity 1.00)
type: system
---
# SQLite Persistence, Durable Progress and Crash Recovery

Download tasks, chunk progress and BT task metadata live in a SQLite database at
`<state_dir>/downloads.db`, opened by `SystemContext::with_components`.
Since the crate split the store is its own crate: `limedl-store` owns the
connection, the schema/migrations and the repositories, while the *callers* —
the 300 ms progress cycle and the scheduler's rebalance persist — live one layer
up in `limedl-engine::persistence`, because they name `download::ManagedDownload`
which sits above the store. The full upsert on state transitions and the
load-back-into-memory path are inherent methods on `DownloadManager` in
`limedl-core`.
[[cite:crates/limedl-engine/src/context.rs:L58-L59]]
[[cite:crates/limedl-store/src/lib.rs:L1-L21]]
[[cite:crates/limedl-engine/src/persistence.rs:L1-L21]]
[[cite:crates/limedl-engine/src/manager.rs:L1101-L1115]]

## Dual connections and PRAGMAs

`Database` holds two `Arc<Mutex<Connection>>`: `write_conn` and `read_conn`.
`open(path)` configures the writer with:

| PRAGMA | Value | Why |
| --- | --- | --- |
| `journal_mode` | `WAL` | readers are not blocked by the writer |
| `wal_autocheckpoint` | 4096 | bound WAL growth |
| `foreign_keys` | `ON` | cascade chunk deletion |
| `busy_timeout` | 5000 | tolerate short lock contention |
| `synchronous` | `NORMAL` | safe with WAL; FULL would force an fsync per checkpoint and double the I/O the buffer pool already does |
| `cache_size` | `-32000` | ~32 MB page cache |

[[cite:crates/limedl-store/src/database/connection.rs:L17-L20]]
[[cite:crates/limedl-store/src/database/connection.rs:L27-L61]]

The reader opens the same file with `query_only = 1` and its own 5 s busy timeout,
so a read path cannot accidentally write. Both connections get a prepared-statement
cache of 64.
[[cite:crates/limedl-store/src/database/connection.rs:L140-L151]]

All DB calls are synchronous and blocking; async callers wrap them in
`spawn_blocking` (the 300 ms cycle) or `block_in_place` (inserting loaded tasks
during startup), and each repository fn holds the connection mutex only for the
duration of its transaction.
[[cite:crates/limedl-engine/src/persistence.rs:L77-L89]]
[[cite:crates/limedl-engine/src/manager.rs:L1183-L1187]]
[[cite:crates/limedl-store/src/database/chunk_repo.rs:L73-L90]]

### Integrity probe and quarantine

Before anything else touches the file, `open` runs `quarantine_if_corrupt`:

- Files above `MAX_INTEGRITY_CHECK_BYTES` (128 MiB) skip the probe, which reads
  every page, so startup stays bounded; an empty or missing file also returns
  early and lets the normal open path report a real problem.
  [[cite:crates/limedl-store/src/database/connection.rs:L12-L15]]
  [[cite:crates/limedl-store/src/database/connection.rs:L319-L336]]
- Otherwise a probe connection (given the writer's 5 s `busy_timeout`, so a
  transient lock is waited out rather than reported) runs `PRAGMA quick_check`.
  `ok` continues normally; any other verdict quarantines.
  [[cite:crates/limedl-store/src/database/connection.rs:L360-L381]]
- `is_unusable_database` classifies by SQLite error code (`NotADatabase`,
  `DatabaseCorrupt`), not by message: a file that is not a database fails its
  first query rather than at open time, so the code path is reached through the
  `quick_check` error.
  [[cite:crates/limedl-store/src/database/connection.rs:L282-L293]]
- `is_lock_contention` matches "locked"/"busy" in the error text and means "someone
  else is using it"; it is **never** treated as corruption — an unlockable open
  error is returned as an error, and a `quick_check` that merely could not get a
  verdict is skipped with a warning.
  [[cite:crates/limedl-store/src/database/connection.rs:L275-L280]]
  [[cite:crates/limedl-store/src/database/connection.rs:L295-L310]]
  [[cite:crates/limedl-store/src/database/connection.rs:L338-L358]]
- `quarantine_database` renames the file to `<db>.corrupt-<unix_ms>` and moves the
  `-wal` / `-shm` sidecars with it — sidecars are named by appending to the whole
  path, which is why the helper extends the file name instead of its extension — so
  the app continues with an empty history, the bytes are preserved for inspection,
  and a stale WAL cannot replay old frames into the fresh database. Download files
  are never touched.
  [[cite:crates/limedl-store/src/database/connection.rs:L234-L273]]
  [[cite:crates/limedl-store/src/database/connection.rs:L312-L318]]

## Migrations

Schema versioning uses `PRAGMA user_version` plus a static `MIGRATIONS` list of
`(version, name, fn)`. On open, migrations with `version > current` run in order,
each bumping `user_version`, and after any migration ran `ANALYZE` refreshes the
planner statistics.
[[cite:crates/limedl-store/src/database/connection.rs:L63-L66]]
[[cite:crates/limedl-store/src/database/connection.rs:L91-L138]]

Two properties make a half-applied migration recoverable:

- **Each migration commits as one transaction** — every DDL statement *and* the
  `user_version` bump, via `transaction_with_behavior(Immediate)`. A failure
  midway (disk full, I/O error, kill) rolls the whole body back, so replaying it
  is safe. Before this, the body committed statement by statement while
  `user_version` still named the old version, and the next start failed with
  "duplicate column name" on every launch thereafter.
  [[cite:crates/limedl-store/src/database/connection.rs:L98-L129]]
- **Every `ADD COLUMN` goes through `add_column_if_missing`**, which is a no-op
  when a `PRAGMA table_info` lookup finds the column. That covers databases
  written by the pre-transaction code and makes an already-applied body harmless.
  [[cite:crates/limedl-store/src/database/schema.rs:L21-L49]]

There is a compatibility shim for databases written by the older code that added
columns without setting `user_version`: before running migrations, and only while
the version is still below 4, the code detects the `chunk_size` / `mirror_urls`
columns and raises the detected version to 2/3. The probe only ever moves the
version **forward** (`detected.max(...)` plus a `> current_version` guard) — it
must not rewind a `user_version` that is already correct. `table_has_column`
queries `PRAGMA table_info` and carries a `debug_assert` whitelist of the tables
callers may name (`downloads`).
[[cite:crates/limedl-store/src/database/connection.rs:L68-L89]]
[[cite:crates/limedl-store/src/database/schema.rs:L4-L19]]

### In-memory test mode

`open_in_memory()` is compiled for tests and the `test-utils` feature. It uses a
single `:memory:` connection shared by both handles (in-memory SQLite does not
allow a second `:memory:` connection to see the same data), runs all migrations,
sets `user_version` to the last migration's version, and deliberately does **not**
enable WAL — doing so causes "database is locked" for the shared connection.
[[cite:crates/limedl-store/src/database/connection.rs:L159-L188]]

## Schema

The initial schema (`version 1`) creates `downloads` — one row per task, with URL,
paths, byte counts, thread-mode fields, state, checksum fields and timestamps —
`chunks` keyed by `(download_id, chunk_index)` with a
`FOREIGN KEY ... ON DELETE CASCADE`, so deleting a download removes its chunks
automatically, and the `alt_svc_cache` table. Later migrations add `chunk_size`,
the mirror columns, cleanup of legacy `sftp:` rows, business indexes on
`downloads(state)`, `downloads(created_at_ms DESC)` and a partial index on
`chunks(claimed_by)`, `priority`, the CDN columns, `expected_checksum`, the
`bt_tasks` table plus its `created_at_ms DESC` index, and
`add_alt_svc_cache`; `MIGRATIONS` currently ends at **v11**.
[[cite:crates/limedl-store/src/database/schema.rs:L51-L107]]
[[cite:crates/limedl-store/src/database/schema.rs:L115-L258]]

## Two serialization mappings

There is deliberately no single wire format shared between SQLite and JSON:

- **SQLite** stores enums as snake_case text and most fields as columns.
  `download_state_to_text` / `text_to_state`, `thread_mode_to_text`,
  `checksum_mode_to_text` and `adaptive_profile_to_text` are the column mappings.
  `mirror_urls` is the exception: a list, so it is stored as a JSON text column.
  [[cite:crates/limedl-store/src/database/manifest_repo.rs:L16-L119]]
  [[cite:crates/limedl-store/src/database/manifest_repo.rs:L126-L128]]
- **JSON** appears only where a consumer needs a blob rather than columns: the
  `bt_tasks.summary_json` cache column holds a serialized `DownloadSummary`. The
  Aria2 layer does not read stored JSON — it rebuilds a snapshot from the row with
  `snapshot_from_manifest`.
  [[cite:crates/limedl-store/src/database/bt_task_repo.rs:L36-L45]]
  [[cite:crates/limedl-types/src/manifest.rs:L201-L205]]

The checksum mapping carries a migration rule: `sha1` and `xxh3_128` rows map to
`ChecksumMode::None` with a warning, because those algorithms were removed and
their stored digest cannot be compared against a supported one — mapping them onto
a live algorithm would fail every re-check for the life of the row; unknown values
still error.
[[cite:crates/limedl-store/src/database/manifest_repo.rs:L77-L99]]

`insert_manifest_row` / `update_manifest_row` use `prepare_cached` with
`named_params!` so values are passed by reference with zero `String` cloning;
`insert_download` writes the download row and its chunk rows together — a new id
inserts row plus all chunks, an existing id updates the row and upserts only the
`dirty` chunks, inside one `BEGIN IMMEDIATE`.
[[cite:crates/limedl-store/src/database/manifest_repo.rs:L126-L194]]
[[cite:crates/limedl-store/src/database/manifest_repo.rs:L330-L394]]

## Adding a checksum algorithm

The supported set is deliberately narrow: BLAKE3 (the default), SHA-256 and
SHA-512. `ChecksumMode::None` is not an algorithm — it means "no verification" —
and the weak or non-cryptographic digests (SHA-1, XXH3-128) were removed because a
checksum mismatch triggers a re-download, so a collision-forgeable digest defeats
the mechanism.
[[cite:crates/limedl-types/src/types/common.rs:L15-L27]]

`ChecksumHasher` wraps the three algorithms and refuses `None`; BLAKE3 output is
`to_hex()`, while the two SHA-2 variants share the module-private `hex_lower()`.
`hash_slices()` is the synchronous fast path for in-memory buffers.
[[cite:crates/limedl-types/src/checksum/mod.rs:L18-L41]]
[[cite:crates/limedl-types/src/checksum/mod.rs:L63-L100]]

Adding one means touching every site below; missing one leaves verification
silently inert at that boundary:

- **Enum and defaults** — add the variant and the default it lands on.
  [[cite:crates/limedl-types/src/types/common.rs:L21-L29]]
  [[cite:crates/limedl-types/src/types/settings.rs:L360-L379]]
- **Hasher** — `ChecksumHasher::new`/`update`/`finalize` and `hash_slices`.
  [[cite:crates/limedl-types/src/checksum/mod.rs:L18-L100]]
- **SQLite text mapping** — `checksum_mode_to_text` / `text_to_checksum_mode`;
  unknown values must still error.
  [[cite:crates/limedl-store/src/database/manifest_repo.rs:L68-L99]]
- **Aria2 RPC** — `parse_checksum_option` accepts the `sha-256` / `sha-512` /
  `blake3` spellings of the `checksum` option.
  [[cite:crates/limedl-rpc/src/options.rs:L431-L463]]
- **Desktop form** — `checksum_to_str` / `str_to_checksum`, the index-aligned
  `combo::CHECKSUMS`, the `SettingsFormData.download_checksum_idx` mirror and the
  `@tr` item list in the download settings tab.
  [[cite:crates/limedl-native/src/bridge/forms/enums.rs:L73-L86]]
  [[cite:crates/limedl-native/src/bridge/forms/combo.rs:L11-L11]]
  [[cite:crates/limedl-native/ui/types.slint:L187-L187]]
  [[cite:crates/limedl-native/ui/components/settings/tab_download.slint:L73-L79]]
- **Catalogs** — every `.po` catalog must carry the same labels.
  [[cite:crates/limedl-native/lang/en/LC_MESSAGES/limedl-native.po:L917-L924]]
  [[cite:crates/limedl-native/lang/zh_CN/LC_MESSAGES/limedl-native.po:L917-L926]]

Two upgrade paths must survive the change. Historical `sha1` / `xxh3_128` rows
map to `None` with a warning, because their digest belongs to a removed algorithm;
mapping them onto a live one would fail every re-check for the life of the row.
A leftover `defaultChecksum` in `settings.json` is rewritten to `blake3` before
deserialization, so one stale enum value cannot quarantine the whole file.
[[cite:crates/limedl-store/src/database/manifest_repo.rs:L77-L99]]
[[cite:crates/limedl-types/src/settings/mod.rs:L522-L545]]

## What gets persisted is the durable progress

A chunk carries two progress counters and the database stores only one of them:

| Counter | Meaning |
| --- | --- |
| `ChunkManifest::downloaded` / `::completed` | bytes **received** into the write buffer; the scheduler, the claim logic and the UI follow these |
| `ChunkManifest::durable_downloaded` (+ `DownloadCore::durable_bytes`) | bytes that provably reached the file, and what gets persisted |

[[cite:crates/limedl-types/src/manifest.rs:L75-L95]]
[[cite:crates/limedl-engine/src/download/managed.rs:L20-L32]]

`record_progress_on_managed` only advances the received pair (deliberately without
setting `dirty`). `record_durable_bytes` is the only writer of the durable pair and
runs when the write buffer reports a successful flush (or a direct write returns);
it credits every chunk the write overlaps and sets `dirty` only when the durable
value actually grew.
[[cite:crates/limedl-engine/src/download/managed.rs:L155-L182]]
[[cite:crates/limedl-engine/src/download/managed.rs:L184-L220]]

`chunk_to_params` therefore writes
`downloaded = chunk.durable_bytes()` and `completed = durable >= chunk.byte_len()`,
and `durable_bytes()` **clamps to the received count**, so a stale or over-eager
counter can only under-report — which costs a re-download, never the other
direction.
[[cite:crates/limedl-store/src/database/chunk_repo.rs:L27-L46]]
[[cite:crates/limedl-types/src/manifest.rs:L110-L124]]

All three writers follow that rule:

- `persist_manifest_snapshot` (the 300 ms cycle in the HTTP executor,
  `PERSIST_INTERVAL`),
- `persist_manifest_snapshots_batch` (the scheduler's rebalance cycle, one
  transaction for all active tasks),
- `DownloadManager::persist` (the full upsert on state transitions).

The first two take their task-level value from
`core.durable_bytes.min(core.manifest.downloaded_bytes)`; the third substitutes it
into the cloned manifest it hands to `insert_download`. Without this, the 300 ms
cycle stored the received counters, which run ahead of the file by whatever the
buffer still held — a hard kill then left the database claiming a chunk was
complete while its bytes were only in memory, and the next resume skipped over the
resulting hole. The incremental writers also retry once on a transient failure and
are guarded by `state NOT IN ('completed','failed','canceled')` so a snapshot taken
just before finalize's full upsert cannot rewind a terminal row.
[[cite:crates/limedl-engine/src/download/shared.rs:L16-L18]]
[[cite:crates/limedl-engine/src/persistence.rs:L23-L71]]
[[cite:crates/limedl-engine/src/persistence.rs:L113-L155]]
[[cite:crates/limedl-engine/src/manager.rs:L1192-L1205]]
[[cite:crates/limedl-store/src/database/chunk_repo.rs:L63-L90]]

The durability boundary is `pwrite`, not `fsync`: a successful flush counts, which
covers a process crash, `SIGKILL` and the OOM killer, but not power loss (those
bytes are in the page cache, and so are the WAL commits, so the two are lost
together). See
[[wiki:systems/disk-io-and-storage]] for the flush-observer contract
that feeds these counters.
[[cite:crates/limedl-engine/src/download/managed.rs:L184-L196]]

## Crash recovery on startup

`DownloadManager::load_downloads_from_db` reconstructs in-memory
`ManagedDownload` entries from stored headers:

1. A task left in `Verifying` with the destination present and the temp file
   gone is promoted to `Completed` (the rename finished, the crash happened
   before the state write).
2. A task left in `Downloading` **stays** `Downloading` but its
   `connection_count` is zeroed and `allocated_thread_count` set to 0, so the
   scheduler re-allocates threads on the next rebalance.
3. `Retrying`, `Verifying` and `Queued` become `Paused`.
4. Completed/Failed/Canceled/Paused are unchanged.
5. Chunks are loaded lazily only for non-terminal tasks, and **every
   `claimed_by` is cleared**. A fresh manager owns no chunk workers, so any
   persisted claim belongs to a process that no longer exists; leaving it would
   make `claim_next_chunk` skip that chunk while the chunk map still reports work
   to do, stalling the resumed download. A cancelled/aborted run releases claims
   through `shutdown_chunk_workers` → `release_all_chunk_claims` before persisting,
   so this is the crash-recovery equivalent.
6. The loaded counters are durable by construction: `row_to_chunk` sets
   `durable_downloaded` from the stored `downloaded` value (at rest received ==
   durable) and `DownloadCore::new` seeds `durable_bytes` from the manifest, so a
   resumed task starts with received == durable and only diverges as new bytes are
   buffered.

[[cite:crates/limedl-engine/src/manager.rs:L1110-L1190]]
[[cite:crates/limedl-store/src/database/chunk_repo.rs:L48-L61]]
[[cite:crates/limedl-engine/src/download/managed.rs:L33-L42]]
[[cite:crates/limedl-engine/src/http_executor/worker.rs:L190-L210]]

## The bt_tasks cache

`bt_tasks(id, summary_json, created_at_ms)` lets the UI show BT tasks while the
librqbit engine is unloaded (lightweight BT mode). `replace_bt_tasks` runs
`BEGIN IMMEDIATE`, deletes all rows and re-inserts the engine's current list in one
transaction, so a crash cannot leave a half-updated index; `DownloadSummary` is
stored as JSON, which means new summary fields need no migration.
`list_bt_tasks` skips a row whose JSON fails to parse with a warning rather than
failing the whole list, and `delete_bt_task` removes one row on cancel/remove/purge.
[[cite:crates/limedl-store/src/database/bt_task_repo.rs:L20-L60]]
[[cite:crates/limedl-store/src/database/bt_task_repo.rs:L62-L97]]

This table is a **cache**, not the source of truth — the engine's on-disk `.resume`
files are; while the session runs, `LazyBtBackend::sync_index_with` stores
`RqbitBtBackend::list()` over the index, and the cached rows (with volatile
transfer metrics zeroed) are only served while the engine is down. The index is
also what decides whether loading the engine at launch is worthwhile, falling back
to the resume files when the index is empty.
[[cite:crates/limedl-store/src/database/bt_task_repo.rs:L1-L12]]
[[cite:crates/limedl-bt/src/lazy.rs:L674-L690]]
[[cite:crates/limedl-bt/src/lazy.rs:L715-L721]]
[[cite:crates/limedl-bt/src/lazy.rs:L313-L329]]

## Terminal-row visibility

`max_in_memory_downloads` evicts tasks from memory only. The database remains the
authority for terminal results, which the Aria2 RPC layer queries through
`get_download_header(id)` (in `summary_for_raw_id`, only accepting a terminal
state), `list_download_headers()` and `count_terminal_downloads()`, until
`aria2.purgeDownloadResult` / `aria2.removeDownloadResult` delete the row — the
latter falls back to removing the database row directly for a GID whose task was
already evicted. `delete_download` removes the download row (chunks cascade via
the FK) and runs the incremental vacuum.
[[cite:crates/limedl-store/src/database/manifest_repo.rs:L405-L433]]
[[cite:crates/limedl-store/src/database/manifest_repo.rs:L436-L445]]
[[cite:crates/limedl-store/src/database/manifest_repo.rs:L515-L530]]
[[cite:crates/limedl-rpc/src/query.rs:L205-L224]]
[[cite:crates/limedl-rpc/src/download.rs:L468-L480]]

## Maintenance

On clean shutdown `Database::shutdown()` runs
`PRAGMA wal_checkpoint(TRUNCATE)` to shrink the WAL file, after the downloads have
been stopped and persisted. `vacuum_if_needed` runs an incremental vacuum only when
the freelist page count exceeds its threshold (100, at the `delete_download` call
site), so normal operation does not pay for a full vacuum; `shrink_memory` flushes
unused page and statement caches on a best-effort `try_lock`.
[[cite:crates/limedl-store/src/database/connection.rs:L198-L231]]
[[cite:crates/limedl-engine/src/manager.rs:L1093-L1097]]

Related pages: [[wiki:workflows/http-download-lifecycle]],
[[wiki:workflows/bit-torrent-backend]],
[[wiki:integrations/aria2-rpc-server]].
