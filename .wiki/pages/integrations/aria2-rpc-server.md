---
id: integrations/aria2-rpc-server
title: Aria2 JSON-RPC Compatibility Server
status: draft
generated_at: 2026-11-09T11:42:32Z
commit: e50d853ddda31dd2196341bd48fb7ca25d84e40c
evidence:
- path: crates/limedl-rpc/Cargo.toml
  lines:
  - 14
  - 18
  blob: 42e51e93cbcb31a1aed11f971714e6d7eb6c1b2d
  snippet_sha1: d4f9efb33d7a8ba231fced8fc4fc8dbe2ae6d013
  note: drift-projected L14-L18@6bad5d87 -> L14-L18 (similarity 1.00)
- path: crates/limedl-native/Cargo.toml
  lines:
  - 20
  - 20
  blob: 22145a24a678bb40d82d9a1e894710e74c1de8aa
  snippet_sha1: 40d1c02f5c3a38a27fb5a2194c83fe9b9b8663da
- path: crates/limedl-server/Cargo.toml
  lines:
  - 22
  - 24
  blob: 6893e6c30f7243eaf34a51bb537afe0a24c4692d
  snippet_sha1: efb2987318d9db3ce58c04197ed22520f006dd91
- path: crates/limedl-rpc/src/protocol.rs
  lines:
  - 14
  - 17
  blob: 8e71eae973eacf356078bed59191bbd416a6c35f
  snippet_sha1: 1259ca85e2e79cb1d7ba0b0d74bef7e73a69d1f5
  note: content lost at HEAD; re-pinned at recorded range (draft)
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 132
  - 160
  blob: 85b9d6828ab075dc9ac2e5e79e68701c2297a5a7
  snippet_sha1: 894687cb0c405fa7e49b93b44bf6e3f755aea0c7
  note: drift-projected L132-L158@90bfb55a -> L132-L160 (similarity 1.00)
- path: crates/limedl-server/src/lib.rs
  lines:
  - 1
  - 10
  blob: 1ad9a05546db8ebc2625bf165a1fbec487eb6375
  snippet_sha1: 99d2fbb5961f9b1cee1fe9fe4ca89864276fb395
  note: drift-projected L1-L10@db33f43e -> L1-L10 (similarity 1.00)
- path: crates/limedl-rpc/src/context.rs
  lines:
  - 56
  - 72
  blob: b1ae86f5e587929b4f7cd8e86433e4def5748666
  snippet_sha1: bd2a45d32e37caec8005f148ae49ad1d32e44e44
  note: drift-projected L56-L69@90629ec9 -> L56-L72 (similarity 1.00)
- path: crates/limedl-types/src/types/settings.rs
  lines:
  - 569
  - 574
  blob: d36d3f34aad95b687828c40c0c27eaae53fccddb
  snippet_sha1: f5f0cb0e992e7b68782f3e2277f667a377d8f464
- path: crates/limedl-rpc/src/bind.rs
  lines:
  - 30
  - 38
  blob: c2525356e5129848d753a132fc7e36b75c77c109
  snippet_sha1: 3854c2c0c50d7a9049812781a6d55980766efccc
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 179
  - 221
  blob: 85b9d6828ab075dc9ac2e5e79e68701c2297a5a7
  snippet_sha1: 1dd84fae1c8eb12e26441df45fdcace8b5f4210d
  note: drift-projected L177-L216@90bfb55a -> L179-L221 (similarity 1.00)
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 120
  - 129
  blob: 85b9d6828ab075dc9ac2e5e79e68701c2297a5a7
  snippet_sha1: 6822fd0664f133531bb8e0256ff37e6409dba337
  note: drift-projected L120-L129@90bfb55a -> L120-L129 (similarity 1.00)
- path: crates/limedl-types/src/types/settings.rs
  lines:
  - 613
  - 618
  blob: d36d3f34aad95b687828c40c0c27eaae53fccddb
  snippet_sha1: df1d33f2490893e4fb9b79842890cec1ccdb8147
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 56
  - 73
  blob: 85b9d6828ab075dc9ac2e5e79e68701c2297a5a7
  snippet_sha1: 93d640b4768c368f3e0d8ef688abab86cb88afea
  note: drift-projected L56-L73@90bfb55a -> L56-L73 (similarity 1.00)
- path: crates/limedl-rpc/src/bind.rs
  lines:
  - 14
  - 28
  blob: c2525356e5129848d753a132fc7e36b75c77c109
  snippet_sha1: f78cc425fde9063449984fb25cc27d6c31145cbb
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 406
  - 446
  blob: 2361c059cc2d5b4fa1ca688ea8acdfce132258e7
  snippet_sha1: 82935c62ffe75562add507d9343f2c6116034733
  note: renamed crates/limedl-core/src/manager.rs -> crates/limedl-engine/src/manager.rs, drift-projected L408-L448 -> L406-L446 (similarity 1.00)
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 56
  - 61
  blob: 85b9d6828ab075dc9ac2e5e79e68701c2297a5a7
  snippet_sha1: e56147f90fecec32e413989f54a11636325b8a49
  note: drift-projected L56-L61@90bfb55a -> L56-L61 (similarity 1.00)
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 75
  - 118
  blob: 85b9d6828ab075dc9ac2e5e79e68701c2297a5a7
  snippet_sha1: e5e490d4d3ef1f7a4d9473d423a0f06fc3090926
  note: drift-projected L75-L118@90bfb55a -> L75-L118 (similarity 1.00)
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 85
  - 91
  blob: 85b9d6828ab075dc9ac2e5e79e68701c2297a5a7
  snippet_sha1: 2f7d7b8a705f05ec9965c8831b952fa673cc2fa3
  note: drift-projected L85-L91@90bfb55a -> L85-L91 (similarity 1.00)
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 93
  - 117
  blob: 85b9d6828ab075dc9ac2e5e79e68701c2297a5a7
  snippet_sha1: 6a80d7841ce3881e4fe91b581ec6e88cc4e52d80
  note: drift-projected L93-L117@90bfb55a -> L93-L117 (similarity 1.00)
- path: crates/limedl-rpc/src/system.rs
  lines:
  - 45
  - 63
  blob: 2755930eae5317d6b8d1989fba760f6513fd0f69
  snippet_sha1: 7ab7b7e0aca8031b5d35c084816897e7ba0344b4
- path: crates/limedl-types/src/types/settings.rs
  lines:
  - 639
  - 643
  blob: d36d3f34aad95b687828c40c0c27eaae53fccddb
  snippet_sha1: f432f0514a3f9c3b31c433eda6f623ea34f7b633
- path: crates/limedl-server/src/lib.rs
  lines:
  - 90
  - 92
  blob: 1ad9a05546db8ebc2625bf165a1fbec487eb6375
  snippet_sha1: 6293fe43264a5d39e3abc505f170c64cd30da9ae
  note: drift-projected L88-L90@db33f43e -> L90-L92 (similarity 1.00)
- path: crates/limedl-server/src/lib.rs
  lines:
  - 106
  - 154
  blob: 1ad9a05546db8ebc2625bf165a1fbec487eb6375
  snippet_sha1: bf0af530b4641089487c84c554e70582ce13bc00
  note: drift-projected L104-L139@db33f43e -> L106-L154 (similarity 0.89)
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 168
  - 177
  blob: 85b9d6828ab075dc9ac2e5e79e68701c2297a5a7
  snippet_sha1: 39cfc57e2572e077d6b1804b3eec626fe0660487
  note: drift-projected L166-L175@90bfb55a -> L168-L177 (similarity 1.00)
- path: crates/limedl-types/src/types/settings.rs
  lines:
  - 646
  - 664
  blob: d36d3f34aad95b687828c40c0c27eaae53fccddb
  snippet_sha1: 80a8d653240eeeaf418b740c51bcf8d9b936858b
- path: crates/limedl-server/src/lib.rs
  lines:
  - 87
  - 89
  blob: 1ad9a05546db8ebc2625bf165a1fbec487eb6375
  snippet_sha1: a2b5a8c601841b66a8c7791b3089a2503cc279b6
  note: drift-projected L85-L87@db33f43e -> L87-L89 (similarity 1.00)
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 187
  - 197
  blob: 85b9d6828ab075dc9ac2e5e79e68701c2297a5a7
  snippet_sha1: 2921b7982b8fabce2710e149056bc2ed46f82b17
  note: drift-projected L185-L195@90bfb55a -> L187-L197 (similarity 1.00)
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 19
  - 54
  blob: 85b9d6828ab075dc9ac2e5e79e68701c2297a5a7
  snippet_sha1: e8de6cc930c07bf1c8dd4b5433b030212f83fb4e
  note: drift-projected L19-L54@90bfb55a -> L19-L54 (similarity 1.00)
- path: crates/limedl-rpc/src/context.rs
  lines:
  - 17
  - 49
  blob: b1ae86f5e587929b4f7cd8e86433e4def5748666
  snippet_sha1: 076405177a7d265fb39c5b1abfda5ef4b166bab1
  note: drift-projected L17-L49@90629ec9 -> L17-L49 (similarity 1.00)
- path: crates/limedl-rpc/src/token.rs
  lines:
  - 173
  - 176
  blob: f99fd37831be64aa6ad9b446e2136764080cdf15
  snippet_sha1: 78379b2623e3657ad287337a5fd7d3e337333307
  note: drift-projected L125-L128@9c68d362 -> L173-L176 (similarity 1.00)
- path: crates/limedl-types/src/types/settings.rs
  lines:
  - 583
  - 604
  blob: d36d3f34aad95b687828c40c0c27eaae53fccddb
  snippet_sha1: 7988e2644bf722391ab5c9bb7462e0149ee31f5f
- path: crates/limedl-rpc/src/context.rs
  lines:
  - 34
  - 47
  blob: b1ae86f5e587929b4f7cd8e86433e4def5748666
  snippet_sha1: 36e00508b50e8df65746eff29a6a55a64ce1e1ec
  note: drift-projected L34-L47@90629ec9 -> L34-L47 (similarity 1.00)
- path: crates/limedl-rpc/src/token.rs
  lines:
  - 67
  - 77
  blob: f99fd37831be64aa6ad9b446e2136764080cdf15
  snippet_sha1: 2a1cce87e4257f87ab6c98e2e5bdd7d9ba7d603b
  note: drift-projected L62-L70@9c68d362 -> L67-L77 (similarity 0.89)
- path: crates/limedl-rpc/src/token.rs
  lines:
  - 144
  - 149
  blob: f99fd37831be64aa6ad9b446e2136764080cdf15
  snippet_sha1: edf5093e3834939a8e44b9eb9b8b526048af449c
  note: drift-projected L96-L101@9c68d362 -> L144-L149 (similarity 1.00)
- path: crates/limedl-rpc/src/token.rs
  lines:
  - 151
  - 161
  blob: f99fd37831be64aa6ad9b446e2136764080cdf15
  snippet_sha1: 6b5629d7bd3fcf0336e57ce8ec0b864ccec9fb21
  note: drift-projected L103-L113@9c68d362 -> L151-L161 (similarity 1.00)
- path: crates/limedl-types/src/types/settings.rs
  lines:
  - 599
  - 600
  blob: d36d3f34aad95b687828c40c0c27eaae53fccddb
  snippet_sha1: ec00054240433a178369c63d9debf9b47c280e27
- path: crates/limedl-rpc/src/token.rs
  lines:
  - 163
  - 171
  blob: f99fd37831be64aa6ad9b446e2136764080cdf15
  snippet_sha1: 893ce0cd1d6d540af8b3c9e8924ec0b2241f9c3a
  note: drift-projected L115-L123@9c68d362 -> L163-L171 (similarity 1.00)
- path: crates/limedl-rpc/src/token.rs
  lines:
  - 12
  - 16
  blob: f99fd37831be64aa6ad9b446e2136764080cdf15
  snippet_sha1: 97a9882ed342bb7ef69bc740301ac0c982551332
  note: drift-projected L12-L16@9c68d362 -> L12-L16 (similarity 1.00)
- path: crates/limedl-rpc/src/token.rs
  lines:
  - 79
  - 98
  blob: f99fd37831be64aa6ad9b446e2136764080cdf15
  snippet_sha1: f62416132b4e2f2eaf676e5a79984feb7dd64182
  note: drift-projected L72-L91@9c68d362 -> L79-L98 (similarity 1.00)
- path: crates/limedl-rpc/src/dispatch.rs
  lines:
  - 26
  - 54
  blob: b8c503f0425740376583770d2258220510ac392d
  snippet_sha1: 6188a0747611c99b538fe2505d0308840b2a7f1d
  note: drift-projected L23-L51@d510ae51 -> L26-L54 (similarity 0.97)
- path: crates/limedl-rpc/src/context.rs
  lines:
  - 98
  - 113
  blob: b1ae86f5e587929b4f7cd8e86433e4def5748666
  snippet_sha1: c67639c5429cb4a28e895085085670bc5260cb1f
  note: drift-projected L95-L110@90629ec9 -> L98-L113 (similarity 0.94)
- path: crates/limedl-rpc/src/dispatch.rs
  lines:
  - 39
  - 51
  blob: b8c503f0425740376583770d2258220510ac392d
  snippet_sha1: 13952e387363cdc1d65cb5c40ecd57f89aec64ae
  note: drift-projected L36-L48@d510ae51 -> L39-L51 (similarity 1.00)
- path: crates/limedl-rpc/src/dispatch.rs
  lines:
  - 106
  - 148
  blob: b8c503f0425740376583770d2258220510ac392d
  snippet_sha1: c08ede0ce017ca45bc83e1e4cad3dc79bdf93801
  note: drift-projected L103-L131@d510ae51 -> L106-L148 (similarity 1.00)
- path: crates/limedl-rpc/src/protocol.rs
  lines:
  - 41
  - 46
  blob: 8e71eae973eacf356078bed59191bbd416a6c35f
  snippet_sha1: 6a0d6540a73ddf794af9ec0dc420711c4cc923b8
  note: drift-projected L42-L47@33c33aea -> L41-L46 (similarity 1.00)
- path: crates/limedl-rpc/src/protocol.rs
  lines:
  - 25
  - 70
  blob: 8e71eae973eacf356078bed59191bbd416a6c35f
  snippet_sha1: 5cb338a19e858f843cc532be7f85b35405fdd0f6
  note: drift-projected L26-L71@33c33aea -> L25-L70 (similarity 1.00)
- path: crates/limedl-rpc/src/protocol.rs
  lines:
  - 18
  - 24
  blob: 8e71eae973eacf356078bed59191bbd416a6c35f
  snippet_sha1: 16aab8567474949007a9d928e047231f85433b74
  note: drift-projected L19-L25@33c33aea -> L18-L24 (similarity 1.00)
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 134
  - 136
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: 1d3ce81b536b1f52f661481f5f11ba93459be1af
  note: drift-projected L134-L136@6d4ba265 -> L134-L136 (similarity 1.00)
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 423
  - 426
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: 57b9a3d96d0c4c541c3dfbda2b50b74bcaa85d84
  note: drift-projected L429-L432@6d4ba265 -> L423-L426 (similarity 1.00)
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 531
  - 538
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: 744e757576fa14dbeb906e99307aead016f88d58
  note: drift-projected L539-L546@6d4ba265 -> L531-L538 (similarity 1.00)
- path: crates/limedl-rpc/src/dispatch.rs
  lines:
  - 61
  - 104
  blob: b8c503f0425740376583770d2258220510ac392d
  snippet_sha1: b7813fc0e058281f76fe15d1d2149a11b1da6ec9
  note: drift-projected L58-L101@d510ae51 -> L61-L104 (similarity 1.00)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 583
  - 628
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: 06753562867cbae6285978845e051c33d42ab2d5
  note: drift-projected L592-L637@b8cd589f -> L583-L628 (similarity 1.00)
- path: docs/aria2-interop-testing.md
  lines:
  - 129
  - 131
  blob: 57ce0f0b76a3264bc7db7d8bd38a045e73c81d49
  snippet_sha1: 343e984ab301c0826fd02c072cbea1cbad5e2eb4
- path: crates/limedl-rpc/src/interop_tests.rs
  lines:
  - 80
  - 121
  blob: 1a01fe705420346e5e20f2da82b495da59ea9c42
  snippet_sha1: 131e715e427040882e738ea30d10831decc9363a
- path: docs/aria2-interop-testing.md
  lines:
  - 191
  - 191
  blob: 57ce0f0b76a3264bc7db7d8bd38a045e73c81d49
  snippet_sha1: 0e19ee4ab21d925ed21b8585170f7b21b8d9682c
- path: crates/limedl-rpc/src/dispatch.rs
  lines:
  - 143
  - 153
  blob: b8c503f0425740376583770d2258220510ac392d
  snippet_sha1: a8b4eed1a1ca9ef3a8ab03999797f78ac8f5510a
  note: drift-projected L126-L136@d510ae51 -> L143-L153 (similarity 1.00)
- path: crates/limedl-rpc/src/interop_tests.rs
  lines:
  - 123
  - 154
  blob: 1a01fe705420346e5e20f2da82b495da59ea9c42
  snippet_sha1: c000da169905ac36cbbedd6355bb989a06cc3636
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 19
  - 31
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: 7e5ed790d89b51f6446a0f3b570e992bfccd66ac
  note: drift-projected L19-L31@6d4ba265 -> L19-L31 (similarity 1.00)
- path: crates/limedl-types/src/types/task.rs
  lines:
  - 252
  - 286
  blob: cd48c61d6464160971b95ab16a139dc5d6e65232
  snippet_sha1: 8c6c7869ea6976e8892535a2cc3e92f408d92b0e
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 51
  - 61
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: 6ef8140a03082f4248aee98cf8b02a238a21d456
  note: drift-projected L51-L61@6d4ba265 -> L51-L61 (similarity 1.00)
- path: crates/limedl-rpc/src/options.rs
  lines:
  - 399
  - 419
  blob: 3bf801ef8406717e6ada43b881f3036601097eb8
  snippet_sha1: f469d8cb4db7ea90e5b6f753c3ab31b136cf9090
- path: crates/limedl-rpc/src/options.rs
  lines:
  - 423
  - 429
  blob: 3bf801ef8406717e6ada43b881f3036601097eb8
  snippet_sha1: b1285d39b3b43d5c4ed9436c97a73aa6712dccdf
- path: crates/limedl-rpc/src/options.rs
  lines:
  - 431
  - 464
  blob: 3bf801ef8406717e6ada43b881f3036601097eb8
  snippet_sha1: 87d46eeb1de57b074f3676577dd83af908faa438
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 65
  - 93
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: 59a3f071c19c332f7fd548aed28d15b3d7a9ea0d
  note: drift-projected L65-L93@6d4ba265 -> L65-L93 (similarity 1.00)
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 129
  - 132
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: c3a9c4888b9ec3d0150aef0672fa58c4670941ae
  note: drift-projected L129-L132@6d4ba265 -> L129-L132 (similarity 1.00)
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 95
  - 112
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: bfc1102a3750fb02084a2ce636e6ab6dae6d347f
  note: drift-projected L95-L112@6d4ba265 -> L95-L112 (similarity 1.00)
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 805
  - 820
  blob: 2361c059cc2d5b4fa1ca688ea8acdfce132258e7
  snippet_sha1: 1a56344dc3a0c6c073e015bd5825637dd2c14041
  note: renamed crates/limedl-core/src/manager.rs -> crates/limedl-engine/src/manager.rs, drift-projected L818-L833 -> L805-L820 (similarity 1.00)
- path: crates/limedl-rpc/src/options.rs
  lines:
  - 302
  - 344
  blob: 3bf801ef8406717e6ada43b881f3036601097eb8
  snippet_sha1: 3f63dac296c6ed3d69ac5d0171b03cec45c2bacd
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 121
  - 127
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: eb3c3e1374776242081d97c8f144e8da4e9e2ec9
  note: drift-projected L121-L127@6d4ba265 -> L121-L127 (similarity 1.00)
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 167
  - 216
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: 818aecdcdc4935f4413117a39266faa34af0b657
  note: drift-projected L170-L219@6d4ba265 -> L167-L216 (similarity 1.00)
- path: docs/aria2-interop-testing.md
  lines:
  - 181
  - 181
  blob: 57ce0f0b76a3264bc7db7d8bd38a045e73c81d49
  snippet_sha1: 09b64b113588b67bfa1aaf490946c5fcbf4242d6
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 227
  - 276
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: c0260ba28ca930dec284d510ea692198de653c99
  note: drift-projected L230-L279@6d4ba265 -> L227-L276 (similarity 1.00)
- path: crates/limedl-engine/src/metalink/types.rs
  lines:
  - 58
  - 63
  blob: 1642915c3707abf4e0de6ed6d845eedb9e1e1953
  snippet_sha1: 5ff6aff3e6118154b80986faaf89c8a19f4824da
- path: crates/limedl-engine/src/metalink/types.rs
  lines:
  - 30
  - 56
  blob: 1642915c3707abf4e0de6ed6d845eedb9e1e1953
  snippet_sha1: ddb3b89b4726d42156ebfc751e313512028251bb
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 315
  - 320
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: b2adefe15241dccbe20cb51e9bc7dcbb7fbcd174
  note: drift-projected L318-L323@6d4ba265 -> L315-L320 (similarity 1.00)
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 256
  - 337
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: efe1d0c9756d90d91be2dd9d8b3720d8601bef87
  note: drift-projected L259-L343@6d4ba265 -> L256-L337 (similarity 0.96)
- path: crates/limedl-rpc/src/options.rs
  lines:
  - 864
  - 948
  blob: 3bf801ef8406717e6ada43b881f3036601097eb8
  snippet_sha1: edc942431af1bfb8fecd3036c64a8b97e2ca969a
- path: crates/limedl-rpc/src/options.rs
  lines:
  - 346
  - 377
  blob: 3bf801ef8406717e6ada43b881f3036601097eb8
  snippet_sha1: 58f59c24411262f7b4f7af78c8394fa981aa7079
- path: crates/limedl-rpc/src/protocol.rs
  lines:
  - 128
  - 213
  blob: 8e71eae973eacf356078bed59191bbd416a6c35f
  snippet_sha1: 0e3628416f3b823ef81128853ed1e22b681e7abf
  note: drift-projected L129-L214@33c33aea -> L128-L213 (similarity 1.00)
- path: crates/limedl-rpc/src/protocol.rs
  lines:
  - 85
  - 91
  blob: 8e71eae973eacf356078bed59191bbd416a6c35f
  snippet_sha1: 379a7dcd08bc2dddb3613e2e3c5cbc153f2688d4
  note: drift-projected L86-L92@33c33aea -> L85-L91 (similarity 1.00)
- path: crates/limedl-rpc/src/protocol.rs
  lines:
  - 186
  - 203
  blob: 8e71eae973eacf356078bed59191bbd416a6c35f
  snippet_sha1: 434f461dc9ba433e80a3539b9dea4f2a9aaae69a
  note: drift-projected L187-L204@33c33aea -> L186-L203 (similarity 1.00)
- path: crates/limedl-rpc/src/protocol.rs
  lines:
  - 215
  - 225
  blob: 8e71eae973eacf356078bed59191bbd416a6c35f
  snippet_sha1: 2c606562f60c1affa72a57305b3c3c04fcc48e27
  note: drift-projected L216-L226@33c33aea -> L215-L225 (similarity 1.00)
- path: crates/limedl-rpc/src/protocol.rs
  lines:
  - 132
  - 137
  blob: 8e71eae973eacf356078bed59191bbd416a6c35f
  snippet_sha1: e55abef5601ae604c99f59c2342be47c4c675d02
  note: drift-projected L133-L138@33c33aea -> L132-L137 (similarity 1.00)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 69
  - 80
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: 3ab62b5544f1ad5fa10db2f67d7b325e951da979
  note: drift-projected L69-L80@b8cd589f -> L69-L80 (similarity 1.00)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 264
  - 268
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: c406a41e65bba09c63c0a73018a53d7c79d2f105
  note: drift-projected L267-L271@b8cd589f -> L264-L268 (similarity 1.00)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 50
  - 92
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: 932480773e0e9e3ef5286b5bd17d393301387a6c
  note: drift-projected L50-L92@b8cd589f -> L50-L92 (similarity 1.00)
- path: crates/limedl-rpc/src/protocol.rs
  lines:
  - 105
  - 126
  blob: 8e71eae973eacf356078bed59191bbd416a6c35f
  snippet_sha1: 8a3c8621d1ff548384fcb461965fbdd9fdf8b5fb
  note: drift-projected L106-L127@33c33aea -> L105-L126 (similarity 1.00)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 93
  - 142
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: b80d4999961f6a0f94b11982ac1581a47b9be8fd
  note: drift-projected L93-L142@b8cd589f -> L93-L142 (similarity 1.00)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 189
  - 204
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: 5a387107e3b3734c57c790712cae102e1ffe1de4
  note: drift-projected L189-L204@b8cd589f -> L189-L204 (similarity 1.00)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 400
  - 416
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: eecd17ff0cdccb0db9e6bfb8e3a777a2dbdb0ec0
  note: drift-projected L409-L425@b8cd589f -> L400-L416 (similarity 1.00)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 472
  - 511
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: 20a5ea8ca5144b5dbc437c526046f0a5f350cf75
  note: drift-projected L481-L520@b8cd589f -> L472-L511 (similarity 1.00)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 144
  - 171
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: 26aef7c7f4ffab72119aa45f50c8f29fcc558a6b
  note: drift-projected L144-L171@b8cd589f -> L144-L171 (similarity 1.00)
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 582
  - 630
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: 8eaf08f9662d0ae2e316e3e3cc2b8c42273bb2fa
  note: drift-projected L590-L638@6d4ba265 -> L582-L630 (similarity 1.00)
- path: crates/limedl-engine/src/manager.rs
  lines:
  - 839
  - 915
  blob: 2361c059cc2d5b4fa1ca688ea8acdfce132258e7
  snippet_sha1: a458ba9c07b0294a55ac3bc06b86039490f61b99
  note: renamed crates/limedl-core/src/manager.rs -> crates/limedl-engine/src/manager.rs, drift-projected L852-L931 -> L839-L915 (similarity 0.95)
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 609
  - 620
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: f7751e02699b1d12f016d7732cf37a1450e6d801
  note: drift-projected L617-L628@6d4ba265 -> L609-L620 (similarity 1.00)
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 632
  - 716
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: 9e9550a1c43c50460787ea7f4ffbf6df0faafe46
  note: drift-projected L640-L724@6d4ba265 -> L632-L716 (similarity 1.00)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 285
  - 301
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: b8db67d01e56e5bd8ac0a0f2fe06e997f7b163db
  note: drift-projected L288-L304@b8cd589f -> L285-L301 (similarity 1.00)
- path: docs/aria2-interop-testing.md
  lines:
  - 179
  - 179
  blob: 57ce0f0b76a3264bc7db7d8bd38a045e73c81d49
  snippet_sha1: 045cba9b3d2f985443836d29fc43a001d5de23b2
- path: crates/limedl-rpc/src/dispatch.rs
  lines:
  - 96
  - 96
  blob: b8c503f0425740376583770d2258220510ac392d
  snippet_sha1: 53ee69819742679d6d978a0620f8f6136f9c7a18
  note: drift-projected L93-L93@d510ae51 -> L96-L96 (similarity 1.00)
- path: crates/limedl-rpc/src/options.rs
  lines:
  - 16
  - 189
  blob: 3bf801ef8406717e6ada43b881f3036601097eb8
  snippet_sha1: 931b390d50e31b38edccbae87ec2eac317409525
- path: crates/limedl-rpc/src/options.rs
  lines:
  - 191
  - 226
  blob: 3bf801ef8406717e6ada43b881f3036601097eb8
  snippet_sha1: 25c815bd79e7cedc41d748fd938811cbebf23348
- path: crates/limedl-rpc/src/options.rs
  lines:
  - 496
  - 736
  blob: 3bf801ef8406717e6ada43b881f3036601097eb8
  snippet_sha1: b95c978d9daa950b076206e02c3e70ab4c342932
- path: crates/limedl-rpc/src/options.rs
  lines:
  - 750
  - 862
  blob: 3bf801ef8406717e6ada43b881f3036601097eb8
  snippet_sha1: ad95805bbef02ee8bee885e4fea27dad1a3dcc9c
- path: crates/limedl-rpc/src/interop_tests.rs
  lines:
  - 623
  - 654
  blob: 1a01fe705420346e5e20f2da82b495da59ea9c42
  snippet_sha1: 4893c89aeb2a5bf5b3441fc7bc6b9364f113245e
- path: crates/limedl-rpc/src/oracle_tests.rs
  lines:
  - 517
  - 595
  blob: 8f5ea65b8ea7dd966d37dadcc0a1c6f1946b2de5
  snippet_sha1: af841cbf9025b7dcb4e19f81d684f2f9dd1dd7b2
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 202
  - 209
  blob: 85b9d6828ab075dc9ac2e5e79e68701c2297a5a7
  snippet_sha1: 4f9255cde1df455b1736ba157f9aee927ddc34d9
  note: drift-projected L199-L205@90bfb55a -> L202-L209 (similarity 1.00)
- path: crates/limedl-rpc/src/transport.rs
  lines:
  - 146
  - 207
  blob: 2f977c8c126d03f7a6083c9e380aa58d056de91b
  snippet_sha1: 44062785acb503604e3a75246ecff6d57fe9bd30
  note: drift-projected L146-L207@985488dd -> L146-L207 (similarity 1.00)
- path: crates/limedl-rpc/src/transport.rs
  lines:
  - 49
  - 144
  blob: 2f977c8c126d03f7a6083c9e380aa58d056de91b
  snippet_sha1: f59ee13cd668a2b27617085cf95be4b6ed7c4742
  note: drift-projected L49-L144@985488dd -> L49-L144 (similarity 0.99)
- path: crates/limedl-rpc/tests/ws_e2e.rs
  lines:
  - 14
  - 30
  blob: 32ba82613d1cce90e12f14269112d251c34801ec
  snippet_sha1: fee28b85cc453ff6d1288ce13c7db781518cf843
- path: crates/limedl-rpc/src/jsonrpc.rs
  lines:
  - 50
  - 62
  blob: 430d5be38af005159dc093628f4ae58112dbd0e6
  snippet_sha1: 0780e9584d6757753a4d03d805cd4686d7ee7191
- path: docs/aria2-interop-testing.md
  lines:
  - 172
  - 193
  blob: 57ce0f0b76a3264bc7db7d8bd38a045e73c81d49
  snippet_sha1: 7116018027702ff4220cb6d0053f9f2aca94613c
- path: docs/aria2-interop-testing.md
  lines:
  - 191
  - 192
  blob: 57ce0f0b76a3264bc7db7d8bd38a045e73c81d49
  snippet_sha1: e4caf0abd4451b6aeff173b765e42a7083ee9934
- path: docs/aria2-interop-testing.md
  lines:
  - 180
  - 181
  blob: 57ce0f0b76a3264bc7db7d8bd38a045e73c81d49
  snippet_sha1: 6cb176541eb55e0fa18931b431d062deba75069c
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 419
  - 431
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: 206cfe7ae9e9d2d705ff2643b7b30014041fa374
  note: drift-projected L428-L440@b8cd589f -> L419-L431 (similarity 1.00)
- path: crates/limedl-rpc/src/jsonrpc.rs
  lines:
  - 52
  - 62
  blob: 430d5be38af005159dc093628f4ae58112dbd0e6
  snippet_sha1: be111aae764e99da5333020975629a13e9929b70
- path: crates/limedl-rpc/src/oracle_tests.rs
  lines:
  - 1
  - 21
  blob: 8f5ea65b8ea7dd966d37dadcc0a1c6f1946b2de5
  snippet_sha1: 9440bcfafac73675f347ea05f760e6eb2a8e6e5b
- path: crates/limedl-rpc/src/oracle_tests.rs
  lines:
  - 63
  - 71
  blob: 8f5ea65b8ea7dd966d37dadcc0a1c6f1946b2de5
  snippet_sha1: 96e07bef32c8ee0d9c27e45e7bc37a1b5d00111d
- path: .github/workflows/aria2-oracle.yml
  lines:
  - 18
  - 22
  blob: 364518b8f43f7fd0a03443cb5a1c86cc62dc0b34
  snippet_sha1: e78a444f5b3816071e5f95fc36ce63c59d445ec8
- path: .github/workflows/aria2-oracle.yml
  lines:
  - 35
  - 37
  blob: 364518b8f43f7fd0a03443cb5a1c86cc62dc0b34
  snippet_sha1: 7fd734ce36b33676ce0597bbc8dcd12431ed6ac1
- path: crates/limedl-rpc/src/oracle_tests.rs
  lines:
  - 31
  - 52
  blob: 8f5ea65b8ea7dd966d37dadcc0a1c6f1946b2de5
  snippet_sha1: 957a6edba6a68eec3f6b114076506a902ec88204
- path: .github/workflows/aria2-oracle.yml
  lines:
  - 12
  - 17
  blob: 364518b8f43f7fd0a03443cb5a1c86cc62dc0b34
  snippet_sha1: 25f2c9bec6cf86cb48c5ef5ecec2096bc782af69
- path: .github/workflows/aria2-oracle.yml
  lines:
  - 57
  - 67
  blob: 364518b8f43f7fd0a03443cb5a1c86cc62dc0b34
  snippet_sha1: 016e1f82cdae501b87d5a25b10c60a9b497570b2
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 206
  - 227
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: 3d669f8dd9f386e197e0346c02eac4f4eaf6c80e
  note: drift-projected L206-L230@b8cd589f -> L206-L227 (similarity 0.84)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 341
  - 357
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: 4c9f248ee7b54e144bd30cee0f4ac15785c83001
  note: content lost at HEAD; re-pinned at recorded range (draft)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 395
  - 407
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: 94602892bdc7b60b7e3f60821aa11019d8c13ee3
  note: content lost at HEAD; re-pinned at recorded range (draft)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 180
  - 187
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: f499ee1e6bb536e09288880bc1f59caeb5cb0e6e
  note: drift-projected L180-L187@b8cd589f -> L180-L187 (similarity 1.00)
- path: crates/limedl-rpc/src/query.rs
  lines:
  - 229
  - 257
  blob: ff7837757c40c7d02f0b354839dd6f16e7a5703b
  snippet_sha1: 7f534a37c86ae18928c5937ccbbd372a63d569e0
  note: drift-projected L232-L260@b8cd589f -> L229-L257 (similarity 1.00)
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 137
  - 145
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: 25333691800a15cb5021d5624af4034e48536a04
  note: content lost at HEAD; re-pinned at recorded range (draft)
- path: crates/limedl-rpc/src/context.rs
  lines:
  - 148
  - 153
  blob: b1ae86f5e587929b4f7cd8e86433e4def5748666
  snippet_sha1: 0be566e3607c7ce1a3b71710fac81cc1e9c7c5de
  note: drift-projected L145-L150@90629ec9 -> L148-L153 (similarity 1.00)
- path: crates/limedl-bt/src/alerts.rs
  lines:
  - 67
  - 123
  blob: a76fb8f533624b24bc4074a39ac715c662739fff
  snippet_sha1: 072952207f32e71c89786ea9cb28007da71e3d42
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 218
  - 220
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: 4c980ce0a3474ff4a874398973834ef47cfe67ac
  note: drift-projected L221-L223@6d4ba265 -> L218-L220 (similarity 1.00)
- path: crates/limedl-rpc/src/download.rs
  lines:
  - 349
  - 352
  blob: 9e7a873f1188f3b5a204c783ac77c6a2d4b3dd54
  snippet_sha1: 73abb041debe6dba144cf00d9e56c5d2b5c94f66
  note: drift-projected L355-L358@6d4ba265 -> L349-L352 (similarity 1.00)
- path: crates/limedl-rpc/src/context.rs
  lines:
  - 74
  - 89
  blob: b1ae86f5e587929b4f7cd8e86433e4def5748666
  snippet_sha1: 6c0349557dc6568f1877016b653c47fe3ead24ce
  note: drift-projected L71-L86@90629ec9 -> L74-L89 (similarity 1.00)
type: integration
---
# Aria2 JSON-RPC Compatibility Server

`crates/limedl-rpc/src/` implements an aria2-compatible JSON-RPC 2.0 server over
HTTP and WebSocket so clients such as AriaNg and Motrix can drive limedl. It
used to live in `limedl-core` behind an optional `aria2-rpc` feature; it is now
its own crate that sits *above* the engine: it depends on `limedl-core` and
`limedl-core` does not and must not depend on it — that one-way edge is what
keeps `axum`, `tower-http` and `argon2` out of the engine graph
[[cite:crates/limedl-rpc/Cargo.toml:L14-L18]]. Both frontends depend on it
directly: the desktop crate
[[cite:crates/limedl-native/Cargo.toml:L20-L20]] and the headless
`limedl-server` daemon [[cite:crates/limedl-server/Cargo.toml:L22-L24]].
Internal downloads are mapped to aria2 GIDs
[[cite:crates/limedl-rpc/src/protocol.rs:L14-L17]].

The server drives the same `Dispatcher` as the Slint desktop — every handler
routes through it — so the desktop UI and an RPC client stay in sync through the
`EventBus` [[cite:crates/limedl-rpc/src/server.rs:L132-L158]]. For the daemon
the RPC endpoint is the only frontend; see
[[wiki:integrations/headless-server-daemon]]
[[cite:crates/limedl-server/src/lib.rs:L1-L10]].

## Server assembly and lifecycle

`Aria2RpcServer::new(registry, &settings, event_bus)` builds an `RpcContext`
with the registry, an `AuthConfig` derived from settings, a `Dispatcher`, a GID
cache, a fresh session id, the `exit_on_shutdown` flag and an `Arc<Notify>`
shutdown handle [[cite:crates/limedl-rpc/src/server.rs:L132-L158]],
[[cite:crates/limedl-rpc/src/context.rs:L56-L69]]. It resolves the bind address
as `format_bind_addr(settings.listen_address, settings.port)` — `listen_address`
defaults to `127.0.0.1` [[cite:crates/limedl-types/src/types/settings.rs:L569-L574]] —
and `bind.rs` brackets an IPv6 literal so `::1` becomes `[::1]:6800` instead of
the malformed `::1:6800` [[cite:crates/limedl-rpc/src/bind.rs:L30-L38]].

`serve(shutdown)` first enforces the security gate below, then assembles an axum
`Router` with `POST /jsonrpc` (HTTP JSON-RPC) and `GET /jsonrpc` (WebSocket
upgrade), applies the CORS layer built from the settings captured in `new`, and
runs `axum::serve(...).with_graceful_shutdown(...)` driven by the `watch`
receiver [[cite:crates/limedl-rpc/src/server.rs:L177-L216]]. (CORS used to be
passed to `serve` separately from the settings given to `new`; it now lives on
the server so the two cannot disagree
[[cite:crates/limedl-rpc/src/server.rs:L120-L129]].)

### Bind address and the fail-closed authentication gate

`Aria2RpcSettings::listen_address` makes the listener configurable, which is
what lets the daemon serve a LAN
[[cite:crates/limedl-types/src/types/settings.rs:L613-L618]]. A **non-loopback**
address is accepted only together with authentication: `public_bind_rejection`
returns a reason when the host is not loopback and `AuthConfig::is_enabled` is
false, and `serve` refuses to start with that reason
[[cite:crates/limedl-rpc/src/server.rs:L56-L73]]. `is_loopback_bind_address`
accepts `127.0.0.1`, `::1` and `localhost`; anything that does not parse as an
IP (including a hostname it cannot recognize) counts as public, so the gate
fails closed rather than open [[cite:crates/limedl-rpc/src/bind.rs:L14-L28]].

The refusal is not cosmetic. The aria2 `dir` option is not confined to a
download root — it only has to be an absolute path without `..` — so an
anonymous network-reachable endpoint would let anyone who can reach the port
write files anywhere the process can
[[cite:crates/limedl-core/src/manager.rs:L408-L448]],
[[cite:crates/limedl-rpc/src/server.rs:L56-L61]].

### CORS

`build_cors_layer(allowed, allow_any)`
[[cite:crates/limedl-rpc/src/server.rs:L75-L118]]:

- `allow_any_origin = true` emits `Access-Control-Allow-Origin: *` and
  deliberately omits `Access-Control-Allow-Credentials` (browsers reject `*`
  paired with credentials). This is the mode the daemon uses when AriaNg is
  served from a different origin [[cite:crates/limedl-rpc/src/server.rs:L85-L91]].
- Otherwise the parsed `cors_allowed_origins` are echoed; with none configured
  the default is `http://localhost` / `http://127.0.0.1`, and configured origins
  that all fail to parse fall back to localhost with a warning
  [[cite:crates/limedl-rpc/src/server.rs:L93-L117]].

### exit_on_shutdown and the shutdown notify

`aria2.shutdown` / `aria2.forceShutdown` always publishes a `Warning` event and
returns the acknowledgement string, but it only calls
`RpcContext::shutdown_notify.notify_one()` when `exit_on_shutdown` is set
[[cite:crates/limedl-rpc/src/system.rs:L45-L63]]. The desktop leaves that off
(it is a managed subsystem and the UI owns exit)
[[cite:crates/limedl-types/src/types/settings.rs:L639-L643]]; the daemon sets it
[[cite:crates/limedl-server/src/lib.rs:L88-L90]] and awaits
`Aria2RpcServer::shutdown_notify()` before calling `registry.shutdown_all()`
[[cite:crates/limedl-server/src/lib.rs:L104-L139]]. `notify_one` rather than
`notify_waiters` is deliberate: it leaves a permit if the client's shutdown
arrives before the daemon starts awaiting, so the signal is not lost
[[cite:crates/limedl-rpc/src/server.rs:L166-L175]].

**The service is disabled by default.** `Aria2RpcSettings::default()` has
`enabled = false`, because with an empty `secret` the endpoint answers
anonymously and loopback-only binding still lets any local process — and any
page the default CORS policy admits — drive downloads and read destination
paths. Enabling it is an explicit Settings action; an existing `settings.json`
with `"enabled": true` keeps working
[[cite:crates/limedl-types/src/types/settings.rs:L646-L664]]. The headless
daemon overrides `enabled` to `true` (the RPC endpoint is its interface)
[[cite:crates/limedl-server/src/lib.rs:L85-L87]] and consequently must satisfy
the authentication gate whenever it binds non-loopback. When the server does
come up with `AuthConfig::Disabled`, `serve` logs a warning naming the address
and pointing at the secret/per-client options
[[cite:crates/limedl-rpc/src/server.rs:L185-L195]].

### Hot-reload port handoff

Saving settings restarts the RPC server. The predecessor drops its listener on a
different task, so the replacement can observe `AddrInUse`. `bind_with_retry`
retries `AddrInUse` for a 5 s window at 25 ms intervals and returns any other
error immediately; a port genuinely held by another process still fails after
the window [[cite:crates/limedl-rpc/src/server.rs:L19-L54]]. This is why a
settings save does not silently lose the RPC endpoint.

## Authentication

`AuthConfig::from_settings` produces one of three states
[[cite:crates/limedl-rpc/src/context.rs:L17-L49]]:

- `Disabled` — `single` mode with an empty `secret`; every request passes. It is
  reachable in practice only through the legacy `"enabled": true` +
  `"secret": null` combination, since a fresh install starts disabled
  [[cite:crates/limedl-types/src/types/settings.rs:L646-L664]], and a
  non-loopback bind refuses to start in this state
  [[cite:crates/limedl-rpc/src/server.rs:L56-L73]].
- `Shared { secret }` — the legacy single shared secret, compared with
  `subtle::ConstantTimeEq` [[cite:crates/limedl-rpc/src/token.rs:L125-L128]].
- `PerClient(PerClientAuth)` — each configured client has its own Argon2id
  `token_hash` [[cite:crates/limedl-types/src/types/settings.rs:L583-L604]].

An **empty client list in `per_client` mode fails closed** (rejects everything),
because opting into per-client mode and silently degrading to anonymous access
would be the opposite of the intent
[[cite:crates/limedl-rpc/src/context.rs:L34-L47]],
[[cite:crates/limedl-rpc/src/token.rs:L62-L70]].

### Token storage and verification

- `generate_token()` produces 32 random bytes URL-safe base64 (no padding)
  [[cite:crates/limedl-rpc/src/token.rs:L96-L101]].
- `hash_token()` returns an Argon2id PHC string; only that may be persisted.
  `settings.json` never stores plaintext
  [[cite:crates/limedl-rpc/src/token.rs:L103-L113]],
  [[cite:crates/limedl-types/src/types/settings.rs:L599-L600]].
- `verify_token()` returns `false` for a malformed hash rather than erroring, so
  a hand-edited settings file cannot take the server down
  [[cite:crates/limedl-rpc/src/token.rs:L115-L123]].
- Successful verifications are cached in-memory by `BLAKE3(token)` so the
  ~20-40 ms Argon2 cost is paid once per token. The digest is never persisted and
  never accepted on its own — it is only inserted after a full Argon2 success,
  so a settings-file leak cannot be replayed against a running server
  [[cite:crates/limedl-rpc/src/token.rs:L12-L16]],
  [[cite:crates/limedl-rpc/src/token.rs:L72-L91]].

### Centralized validation

`dispatch_method` enforces the token centrally, then routes via
`dispatch_authorized`. Centralizing it is deliberate: with per-handler checks the
`strip_token`/`check_token` order was easy to invert (making a configured secret
reject every legitimate request) and bulk methods such as `pauseAll` silently
skipped authentication [[cite:crates/limedl-rpc/src/dispatch.rs:L23-L51]].
`check_token` only accepts the `token:`-prefixed first parameter, matching
aria2's wire format, so new handlers are protected automatically
[[cite:crates/limedl-rpc/src/context.rs:L95-L110]].

Three method families are exempt from the **outer** check, mirroring aria2:
`system.listMethods`, `system.listNotifications` and the `multicall` aliases
[[cite:crates/limedl-rpc/src/dispatch.rs:L36-L48]].

- `system.listMethods` and `system.listNotifications` are anonymous — they only
  reveal names, and AriaNg calls them without a token even when a secret is set.
- `aria2.multicall`/`system.multicall` carry no outer token in aria2. Each
  nested call is authenticated on its own: `handle_multicall` routes every
  nested call back through `dispatch_method`, so each nested handler checks and
  strips its own `token:` element. An unauthenticated nested call fails with its
  own Unauthorized entry instead of executing, so skipping the outer check is
  not a hole [[cite:crates/limedl-rpc/src/dispatch.rs:L103-L131]].

## GIDs: stable identity across restarts

`internal_id_to_gid(raw_id)` takes the 64-bit prefix of `blake3::hash(raw_id)`
formatted as 16 lowercase hex digits
[[cite:crates/limedl-rpc/src/protocol.rs:L14-L16]]. The underlying task
id is a UUID for HTTP and the info hash for BT, both of which are stable across
restarts, so a GID is stable too
[[cite:crates/limedl-rpc/src/protocol.rs:L42-L47]]. `resolve_gid` checks the
cache first, then scans all backends' `list()` and populates the cache on a hit
— the cache is an optimization, not the source of truth
[[cite:crates/limedl-rpc/src/protocol.rs:L26-L71]]. `resolve_gid` also accepts
an **abbreviated GID**: it prefers an exact match and otherwise resolves a
prefix that identifies exactly one task. A prefix shared by two tasks is refused
rather than guessed, and only full GIDs are cached because a prefix mapping
could go stale when a new task appears
[[cite:crates/limedl-rpc/src/protocol.rs:L19-L25]].

Lifecycle correctness: `addUri` and `resolve_gid`'s scan insert entries
[[cite:crates/limedl-rpc/src/download.rs:L134-L136]], and `aria2.remove`,
`aria2.removeDownloadResult` and `purgeDownloadResult` evict them — otherwise
the cache would grow without bound in a long session
[[cite:crates/limedl-rpc/src/download.rs:L429-L432]],
[[cite:crates/limedl-rpc/src/download.rs:L539-L546]].

## Method routing

`dispatch_authorized` maps method names to handlers: `addUri`/`addTorrent`/
`addMetalink`, `pause`/`forcePause`/`pauseAll`/`forcePauseAll`, `unpause`/`unpauseAll`,
`remove`/`forceRemove`/`removeDownloadResult`/`purgeDownloadResult`,
`tellStatus`/`tellActive`/`tellWaiting`/`tellStopped`, `getGlobalStat`,
`getOption`/`changeOption`, `getGlobalOption`/`changeGlobalOption`, `getFiles`,
`getUris`, `getServers`, `getPeers`, `changeUri`, `changePosition`,
`getSessionInfo`, `saveSession`, `getVersion`, `shutdown`/`forceShutdown`,
`multicall`, plus `system.listMethods`/`system.listNotifications`. Unknown
methods return the method-not-found error code
[[cite:crates/limedl-rpc/src/dispatch.rs:L58-L101]].

`handle_list_methods` advertises exactly the routed set — 37 entries (aria2's
36 plus the `aria2.multicall` alias)
[[cite:crates/limedl-rpc/src/query.rs:L592-L637]],
[[cite:docs/aria2-interop-testing.md:L129-L131]]. The Tier 1 test
`interop_list_methods_matches_the_routed_surface` asserts that equality and
that every advertised method is reachable, so a handler added without a listing
(or vice versa) fails the suite
[[cite:crates/limedl-rpc/src/interop_tests.rs:L80-L121]]. A top-level JSON-RPC
batch is served; the HTTP GET/JSONP transports are not
[[cite:docs/aria2-interop-testing.md:L191-L191]].

### multicall response shape

Each nested result is wrapped in a **single-element array** — `[value]` on
success, `[{"code","message"}]` on failure. AriaNg/Motrix index into element 0,
so a two-element `[null, value]` wrapper would be read as a failure
[[cite:crates/limedl-rpc/src/dispatch.rs:L126-L136]]. Nested calls are
authenticated individually (see centralized validation), and the Tier 1 test
`interop_ariang_multicall_shape` sends them with only per-call tokens, as aria2
clients do [[cite:crates/limedl-rpc/src/interop_tests.rs:L123-L154]].

## URI classification and addUri options

`classify_aria2_uri` routes **only `magnet:` to BT**; an HTTP(S) URL — including
a `.torrent` URL — stays HTTP, because aria2 downloads a `.torrent` URL as a
plain file and uses `addTorrent` for the parsed form. The deeper
`StartDownloadRequest::classify_kind` heuristic (which maps a `.torrent`
extension to BT) is deliberately not used here
[[cite:crates/limedl-rpc/src/download.rs:L19-L31]],
[[cite:crates/limedl-types/src/types/task.rs:L252-L286]].

`handle_add_uri` treats the `uris` array as an ordered candidate list: element 0
is the primary URL and the rest become `mirror_urls` for HTTP
[[cite:crates/limedl-rpc/src/download.rs:L51-L61]]. It translates the aria2
request options into a `StartDownloadRequest`:

| aria2 option | Effect |
| --- | --- |
| `header`, `referer`, `http-user`/`http-passwd` | request headers (Basic auth is synthesized) [[cite:crates/limedl-rpc/src/options.rs:L399-L419]] [[cite:crates/limedl-rpc/src/options.rs:L423-L429]] |
| `checksum` (`TYPE=DIGEST`) | maps to `ChecksumMode` + expected digest (sha-256/sha-512/blake3); an unsupported algorithm is rejected, not silently skipped [[cite:crates/limedl-rpc/src/options.rs:L431-L464]] |
| `user-agent`, `split`, `max-tries`, `out`, `dir` | carried onto the request; `dir` defaults to the configured download dir [[cite:crates/limedl-rpc/src/download.rs:L65-L93]] |
| `pause` | start the task paused (also accepts the string `"true"`) [[cite:crates/limedl-rpc/src/download.rs:L129-L132]] |

A non-terminal HTTP download with the same URL is deduplicated: its existing GID
is returned and cached instead of starting a second task
[[cite:crates/limedl-rpc/src/download.rs:L95-L112]],
[[cite:crates/limedl-core/src/manager.rs:L818-L833]]. `dir` must be absolute and
without `..` components, but it is not confined to a download root — which is
why the bind gate above insists on authentication for a network-reachable
listener [[cite:crates/limedl-core/src/manager.rs:L408-L448]].

Trailing arguments are read **by JSON type**, not position (`split_aria2_tail`
in `options.rs`). aria2's signatures are `addUri(uris[, options[, position]])`
and `addTorrent(torrent[, uris[, options[, position]]])`; AriaNg sends
`addTorrent` as `[torrent, [], options]`. Reading `params[1]` as the options
object therefore mistook AriaNg's empty `uris` array for the options and
silently dropped `dir`/`out`/`pause`/`select-file` — a bug limedl's own
`[torrent, options]` tests could not see
[[cite:crates/limedl-rpc/src/options.rs:L302-L344]]. The type scan accepts both
shapes, and `addUri`/`addTorrent` honour `position` only when it is `0` (front
of the queue → High priority), the same priority approximation `changePosition`
uses [[cite:crates/limedl-rpc/src/download.rs:L121-L127]]. `addTorrent` applies
`dir`/`pause` to the request
[[cite:crates/limedl-rpc/src/download.rs:L170-L219]]; the BT backend does not
yet honour `out`/`select-file` at start (see deviations)
[[cite:docs/aria2-interop-testing.md:L181-L181]].

### addMetalink

`aria2.addMetalink` takes a base64-encoded Metalink XML document: the handler
decodes it, parses it through `metalink::parse_metalink_xml`, and starts one
HTTP task per document file, skipping any file excluded by the `select-file`
option [[cite:crates/limedl-rpc/src/download.rs:L230-L279]]. Each task uses the
best-priority mirror as its primary URL and passes the remaining mirrors as
`mirror_urls` [[cite:crates/limedl-engine/src/metalink/types.rs:L58-L63]], and
pairs them with the file's strongest supported hash (SHA-512 > SHA-256 >
BLAKE3) as the expected checksum
[[cite:crates/limedl-engine/src/metalink/types.rs:L30-L56]]. When the document
carries per-piece hashes they are handed to `set_piece_verification` so the
chunk map can be validated piece by piece
[[cite:crates/limedl-rpc/src/download.rs:L318-L323]]. `dir`, `pause` and
`position` follow the same rules as `addUri`, and the return value is the array
of created GIDs [[cite:crates/limedl-rpc/src/download.rs:L259-L343]]. The
subsystem itself is documented in
[[wiki:workflows/metalink-and-mirror-selection]].

## Runtime option changes

`handle_change_option` deliberately supports a **restricted key set** and errors
with the key name for anything else, instead of silently ignoring it
[[cite:crates/limedl-rpc/src/options.rs:L864-L948]]:

- `pause` (both protocols) — pause when truthy, resume otherwise.
- `select-file` (BT only) — 1-based comma-separated aria2 indices parsed by
  `parse_select_file`, converted to the engine's 0-based indices
  [[cite:crates/limedl-rpc/src/options.rs:L346-L377]].
- `max-download-limit` / `max-upload-limit` (BT only) — the status is read first
  so the other direction is preserved, then `bt_set_speed_limit` is called.

## Status fields (`tellStatus` and the list methods)

`summary_to_aria2_status` (`protocol.rs`) produces the always-present aria2 keys:
`gid`, `status`, `totalLength`, `completedLength`, `uploadLength`,
`downloadSpeed`, `uploadSpeed`, `connections`, `dir` and `files`, plus — for BT
— top-level `infoHash`, `numSeeders` and `seeder`
[[cite:crates/limedl-rpc/src/protocol.rs:L129-L214]]. `errorCode`/`errorMessage`
appear only for terminal states: code `"0"` when there is no error, `"1"`
otherwise, with the human-readable reason in `errorMessage` (limedl does not
persist aria2's specific exit-status codes)
[[cite:crates/limedl-rpc/src/protocol.rs:L86-L92]],
[[cite:crates/limedl-rpc/src/protocol.rs:L187-L204]]. `files[].path` is the
absolute save path, as aria2 reports it
[[cite:crates/limedl-rpc/src/protocol.rs:L216-L226]], and `seeder` describes the
local endpoint (true once the whole payload is present), not connected peers
[[cite:crates/limedl-rpc/src/protocol.rs:L133-L138]].

`enrich_status` adds the fields that need a manifest or backend lookup, on
`tellStatus` and `tellActive` only (the active set is small; `tellWaiting` and
`tellStopped` stay cheap) [[cite:crates/limedl-rpc/src/query.rs:L69-L80]],
[[cite:crates/limedl-rpc/src/query.rs:L267-L271]]:

- HTTP: `bitfield`, `numPieces` and `pieceLength` from the live manifest chunk
  map (`http_pieces`), packed MSB-first by `bitfield_from_bits`
  [[cite:crates/limedl-rpc/src/query.rs:L50-L92]],
  [[cite:crates/limedl-rpc/src/protocol.rs:L106-L127]].
- BT: the `bittorrent` object (`mode` single/multi, `info.name`, `announceList`
  from the trackers) plus `bitfield`/`numPieces` from the piece set. A fresh
  magnet whose metadata has not arrived reports `files: []` and no `bittorrent`
  object, mirroring aria2 [[cite:crates/limedl-rpc/src/query.rs:L93-L142]],
  [[cite:crates/limedl-rpc/src/query.rs:L189-L204]].

`getGlobalStat.uploadSpeed` is the real sum of per-task upload speeds rather
than a constant [[cite:crates/limedl-rpc/src/query.rs:L409-L425]].

## Server, URI and queue-position methods

- `aria2.getServers` (`handle_get_servers`) returns one file-index entry with a
  server struct per URI candidate for HTTP (`uri`/`currentUri`/`downloadSpeed`,
  the live speed on the in-use candidate and `"0"` on the rest), or the tracker
  list for BT [[cite:crates/limedl-rpc/src/query.rs:L481-L520]],
  [[cite:crates/limedl-rpc/src/query.rs:L144-L171]].
- `aria2.changeUri` (`handle_change_uri` plus
  `DownloadManager::change_task_uris`) removes one occurrence per `delUris`
  entry, inserts `addUris` at `position`, refuses to remove the last URI, and
  returns aria2's `[deleted, added]`
  [[cite:crates/limedl-rpc/src/download.rs:L590-L638]],
  [[cite:crates/limedl-core/src/manager.rs:L852-L931]]. `fileIndex` must be `1`
  for limedl's single-file HTTP downloads; BT is rejected
  [[cite:crates/limedl-rpc/src/download.rs:L617-L628]].
- `aria2.changePosition` (`handle_change_position`) maps the requested index
  onto limedl's three priority levels — front to High, back to Low, otherwise
  Normal for `POS_SET`, one level up/down for `POS_CUR` — and returns the
  **actual** resulting index recomputed from `waiting_order`. `tellWaiting` uses
  the same order (priority, then creation time), so the reply matches the queue
  [[cite:crates/limedl-rpc/src/download.rs:L640-L724]],
  [[cite:crates/limedl-rpc/src/query.rs:L288-L304]]. Arbitrary positions among
  more than three same-priority tasks cannot be represented; this is an
  intentional, documented limitation [[cite:docs/aria2-interop-testing.md:L179-L179]].
- `aria2.forceShutdown` routes to the same handler as `aria2.shutdown`, which
  acknowledges and warns [[cite:crates/limedl-rpc/src/dispatch.rs:L93-L93]],
  [[cite:crates/limedl-rpc/src/system.rs:L45-L63]]. It signals the daemon to
  stop when `exit_on_shutdown` is set, but the desktop leaves that off because
  it runs as a managed subsystem and the application UI owns exit
  [[cite:crates/limedl-types/src/types/settings.rs:L639-L643]].

## AriaNg option coverage

`getGlobalOption` answers the complete key set AriaNg's Settings pages read
(`aria2GlobalAvailableOptions` in its `aria2Options.js`, plus the quick-settings
speed limits) [[cite:crates/limedl-rpc/src/options.rs:L16-L189]], and `getOption`
does the same for the task dialog (`aria2TaskAvailableOptions`)
[[cite:crates/limedl-rpc/src/options.rs:L191-L226]]. Values come from limedl's
real settings where the engine has an equivalent (`dir`,
`max-concurrent-downloads`, `split`, `max-tries`, `user-agent`,
`max-overall-*-limit`, the proxy and the mappable `bt-*` / `enable-*` /
`rpc-*` keys) and are documented aria2 defaults otherwise; every value is a
string, as aria2 serialises them
[[cite:crates/limedl-rpc/src/options.rs:L496-L736]]. `changeGlobalOption`
applies the subset it can map (`dir`, `max-concurrent-downloads`, `split`,
`max-connection-per-server`, `max-tries`, `user-agent`, the global speed limits,
`enable-dht`, `bt-max-peers`, `seed-ratio`, `bt-tracker`, `listen-port`) and
ignores unrecognised keys, as aria2 clients expect
[[cite:crates/limedl-rpc/src/options.rs:L750-L862]].

The key sets are pinned by Tier 1
(`interop_get_global_option_covers_ariang_keys`,
`interop_get_option_covers_ariang_task_keys`)
[[cite:crates/limedl-rpc/src/interop_tests.rs:L623-L654]] and by the Tier 2
oracle [[cite:crates/limedl-rpc/src/oracle_tests.rs:L517-L595]].

## Transports and error codes

The endpoint serves `POST /jsonrpc` and `GET /jsonrpc` (WebSocket upgrade)
[[cite:crates/limedl-rpc/src/server.rs:L199-L205]]. A POST body may be a single
request object or a **JSON-RPC 2.0 batch** (a top-level array): each element is
dispatched and answered, notifications (elements without an `id`) execute
without a response entry, an all-notification batch returns no body, and an
empty array is a single Invalid Request
[[cite:crates/limedl-rpc/src/transport.rs:L103-L157]]. The WebSocket transport
accepts the same shapes [[cite:crates/limedl-rpc/src/transport.rs:L30-L101]],
as exercised by `crates/limedl-rpc/tests/ws_e2e.rs`
[[cite:crates/limedl-rpc/tests/ws_e2e.rs:L14-L30]].

Error objects follow aria2 rather than the JSON-RPC-specific codes: unknown
method, missing/invalid params, unknown GID, unauthorized and internal failures
all use `code: 1`, so the method-not-found case is recognized by its message.
Only wire-level parse and version failures keep `-32700`/`-32600`, because they
never reach a method. `ERR_METHOD_NOT_FOUND`/`ERR_INVALID_PARAMS`/`ERR_INTERNAL`
are kept as named constants but all equal 1
[[cite:crates/limedl-rpc/src/jsonrpc.rs:L50-L62]].

## Known, intentional deviations

These are documented choices, not bugs, and are tracked alongside the Tier 1/Tier
2 plan in `docs/aria2-interop-testing.md`
[[cite:docs/aria2-interop-testing.md:L172-L193]]:

- HTTP GET/JSONP transports are not served — POST (including a JSON-RPC batch)
  and WebSocket are. HTTPS RPC and HTTP Basic auth are likewise not served, so a
  LAN deployment needs a reverse proxy for TLS; a configurable bind address
  (aria2's `--rpc-listen-all`) *is* supported, but a non-loopback bind requires
  authentication [[cite:docs/aria2-interop-testing.md:L191-L192]].
- `changePosition` collapses onto the priority model (above)
  [[cite:crates/limedl-rpc/src/download.rs:L640-L724]].
- BitTorrent `tellStatus.dir`/`getOption.dir` report the torrent's destination
  directory (`output_folder`), falling back to the default output dir, aligned
  with HTTP task behaviour; `addTorrent`'s `out`/`select-file` are parsed but
  not applied at start (use `aria2.changeOption` after metadata)
  [[cite:docs/aria2-interop-testing.md:L180-L181]].
- `getOption`/`getGlobalOption` answer AriaNg's full key sets, but several values
  are documented aria2 defaults because the engine has no equivalent. `getVersion`
  is truthful: `Async DNS`, `BitTorrent`, `GZip`, `Brotli`, `Zstd`, `HTTPS`,
  `Message Digest`, `Metalink`, and it deliberately omits aria2's
  `SFTP`/`XML-RPC`/`Firefox3 Cookie` [[cite:crates/limedl-rpc/src/query.rs:L428-L440]].
- Error codes match aria2 (`1` for every domain failure)
  [[cite:crates/limedl-rpc/src/jsonrpc.rs:L52-L62]]; GID prefix matching is
  supported (a unique prefix resolves, an ambiguous one is refused)
  [[cite:crates/limedl-rpc/src/protocol.rs:L19-L25]].

## Tier 2 oracle (live `aria2c`)

`oracle_tests.rs` starts a real `aria2c --enable-rpc` beside limedl's server and
reports how the same requests differ. It is opted in by `ARIA2_ORACLE_BIN`:
unset, each test returns early so the normal gate needs no aria2; set but
unusable, it panics
[[cite:crates/limedl-rpc/src/oracle_tests.rs:L1-L21]],
[[cite:crates/limedl-rpc/src/oracle_tests.rs:L63-L71]].
`.github/workflows/aria2-oracle.yml` runs it as a nightly `schedule` and
`workflow_dispatch` only (never on PRs, Linux only)
[[cite:.github/workflows/aria2-oracle.yml:L18-L22]],
[[cite:.github/workflows/aria2-oracle.yml:L35-L37]].

Differences are **allowlisted, not open-ended**: the oracle asserts that limedl
answers every AriaNg option key, the always-present `tellStatus` keys, and the
documented `listMethods`/`listNotifications` delta, and that both servers answer
a JSON-RPC batch with an array; a run fails on anything outside those allowlists
(and when the oracle cannot start or a server stops answering)
[[cite:crates/limedl-rpc/src/oracle_tests.rs:L31-L52]],
[[cite:.github/workflows/aria2-oracle.yml:L12-L17]]. The remaining aria2-only
differences are printed as allowlisted notes by `--success-output=final`
[[cite:.github/workflows/aria2-oracle.yml:L57-L67]].

## Terminal-result visibility

`max_in_memory_downloads` only evicts tasks from memory. `summary_for_raw_id`
falls back to `db.get_download_header` for a task the HTTP manager no longer
holds, so `tellStatus`/`tellStopped`/`getGlobalStat` remain correct until
`purgeDownloadResult` or `removeDownloadResult` deletes the row
[[cite:crates/limedl-rpc/src/query.rs:L206-L230]]. `tellStopped` also uses
`list_download_headers` [[cite:crates/limedl-rpc/src/query.rs:L341-L357]] and
`getGlobalStat` counts stopped rows with `count_terminal_downloads`
[[cite:crates/limedl-rpc/src/query.rs:L395-L407]].

The `keys` filter is supported on the query methods with aria2's positional
indices (1 for `tellStatus`, 0 for `tellActive`, 2 for
`tellWaiting`/`tellStopped`)
[[cite:crates/limedl-rpc/src/query.rs:L180-L187]],
[[cite:crates/limedl-rpc/src/query.rs:L232-L260]].

## Notification ownership

The most important correctness rule is that each lifecycle transition has exactly
one emitter:

- **HTTP** start/pause/resume/stop notifications are broadcast by the RPC handler
  (`broadcast_event`), because there is no HTTP alert bridge; complete/error
  events come from the executor/lifecycle layer
  [[cite:crates/limedl-rpc/src/download.rs:L137-L145]],
  [[cite:crates/limedl-rpc/src/context.rs:L145-L150]].
- **BT** start/pause/resume/complete/error notifications all come only from the
  BT alert bridge (`crates/limedl-bt/src/alerts.rs`)
  [[cite:crates/limedl-bt/src/alerts.rs:L67-L123]]. The RPC handler therefore
  fires an aria2 notification only when `kind == TaskKind::Http`; doing it for
  BT would duplicate every notification (a historical bug in
  `addTorrent`/`pause`) [[cite:crates/limedl-rpc/src/download.rs:L221-L223]],
  [[cite:crates/limedl-rpc/src/download.rs:L355-L358]].

## The protocol-specific accessor rule

`RpcContext::http()` and `RpcContext::bt()` are the **only** sanctioned
`get_typed` downcasts in the aria2 layer. The RPC surface is inherently
protocol-specific (`getOption` reads the HTTP manifest, `tellStatus.files` asks
the BT engine), so those downcasts are centralized; new handlers must use these
accessors rather than calling `get_typed` directly
[[cite:crates/limedl-rpc/src/context.rs:L71-L86]].

Related pages: [[wiki:architecture/protocol-routing-and-dispatcher]],
[[wiki:workflows/metalink-and-mirror-selection]],
[[wiki:integrations/headless-server-daemon]],
[[wiki:systems/settings-and-configuration]],
[[wiki:workflows/bit-torrent-backend]].
