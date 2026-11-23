# Changelog

## [0.2.3](https://github.com/Soleone/yap/compare/v0.2.2...v0.2.3) (2026-11-23)

### Bug Fixes

* **tray:** tray icon is the bare gold glyph, filling the slot instead of a shrunken tile ([27eb108](https://github.com/Soleone/yap/commit/27eb10817ad6d4a5442bbfc33897d2ed2bbeeb95))

## [0.2.2](https://github.com/Soleone/yap/compare/v0.2.1...v0.2.2) (2026-11-16)

### Features

* **settings:** move token field to General once a token is saved ([6e66f58](https://github.com/Soleone/yap/commit/6e66f589ce857d40a7e281145b163ba85474e891))

## [0.2.1](https://github.com/Soleone/yap/compare/v0.2.0...v0.2.1) (2026-11-16)

### Features

* **settings:** clone voices from a recording or audio file ([8489f5d](https://github.com/Soleone/yap/commit/8489f5d5da0b3750ac7baf6e9b672fc0518fcd56))
* **voices:** clone voices from a recording or clip ([de6dd83](https://github.com/Soleone/yap/commit/de6dd831267e3fbfbc917fc6f4c80f16f67df32d))

## [0.2.0](https://github.com/Soleone/yap/compare/v0.1.5...v0.2.0) (2026-11-16)

### Features

* **breeze:** design each voice once and clone it per piece ([e80eae0](https://github.com/Soleone/yap/commit/e80eae03969a83e2cf9962cfa86a0e6e16dc1523))
* **settings:** allow cancelling a model load ([8b8c019](https://github.com/Soleone/yap/commit/8b8c0193e495a1646af420e34e94badd2e19b3b1))
* **settings:** call the history setting Keep audio files ([871a58c](https://github.com/Soleone/yap/commit/871a58c4edf01f3401459161c401c28dd8e16461))
* **settings:** name single-language Phonon models by their language ([973f0ef](https://github.com/Soleone/yap/commit/973f0ef6889b214ef3aaed8860dae0805fd3663d))
* **settings:** open settings from a gear in a dialog with Voice, Languages, Library and General tabs ([71808ab](https://github.com/Soleone/yap/commit/71808ab6725f4da695751532896d61affedabad1))
* **settings:** read language, model, voice and speed as one chain, and say when a model has one speed ([768ce85](https://github.com/Soleone/yap/commit/768ce85199cdcbb99e4f4e04fdfd2771180d96bc))
* **settings:** show other languages as a list with an Add a language button ([96c9015](https://github.com/Soleone/yap/commit/96c90152cf70d2bf82fb6782a06819ab8b8fe14b))
* **settings:** star voices to find them first in the voice lists ([70977dd](https://github.com/Soleone/yap/commit/70977dddb61c55f481193ee125846d5bad3593e7))

## [0.1.5](https://github.com/Soleone/yap/compare/v0.1.4...v0.1.5) (2026-11-16)

### Features

* **settings:** move models to a new folder without restarting ([521d136](https://github.com/Soleone/yap/commit/521d136df3b8aaeb3c1e66833f954781715fcf7c))

## [0.1.4](https://github.com/Soleone/yap/compare/v0.1.3...v0.1.4) (2026-11-16)

### Features

* **settings:** explain gated model failures and offer retry ([4826ecc](https://github.com/Soleone/yap/commit/4826ecc39af8f179a6b288fb7850176274916de8))

### Bug Fixes

* **update:** recheck for updates when the window gains focus ([55785b6](https://github.com/Soleone/yap/commit/55785b61a9234ddd1734c97ea91462dd946eb50f))

## [0.1.3](https://github.com/Soleone/yap/compare/v0.1.2...v0.1.3) (2026-11-16)

### Features

* **speak:** read text in the voice of its language ([2793a08](https://github.com/Soleone/yap/commit/2793a08dedd6c70ad7ed8cb5872b85bdcc37ce20))

## [0.1.2](https://github.com/Soleone/yap/compare/v0.1.1...v0.1.2) (2026-11-15)

### Bug Fixes

* leave CHANGELOG.md out of formatting, like Pluck ([b4321e4](https://github.com/Soleone/yap/commit/b4321e422320520757736d53b58c03eedb47de80))

## 0.1.1 (2026-11-15)

### Features

* --source-env takes where the text came from out of the caller's environment; build like Tasks and Reader ([e67607f](https://github.com/Soleone/yap/commit/e67607f3436b48b53665dc9b3209336ddf43ba7a))
* Kokoro, Breeze, Qwen3 and Ming through Python workers; language before model; yap --save; Keep reads and pruning ([abbb5cf](https://github.com/Soleone/yap/commit/abbb5cf3767c4ac10845a2641982af60d2b55290))
* library window to replay, seek and delete reads ([0e8f139](https://github.com/Soleone/yap/commit/0e8f1392cf6925bba9fa38521ac3d41026a8776a))
* models folder setting with the system folder picker, applied at restart ([9ced7b9](https://github.com/Soleone/yap/commit/9ced7b96b934ee856602a6531199ea41e49d6c6a))
* optional hotkey that reads the selected text of any app ([c25ad53](https://github.com/Soleone/yap/commit/c25ad5328b0c9e45c42313f9ebf78e112ff92bb0))
* pnpm release and a signed GitHub release with in-app updates, like Pluck ([f5a6309](https://github.com/Soleone/yap/commit/f5a630925855a11f2d57b9af7433b887232d426d))
* settings for voice, speed, model, popup and startup ([3e540a1](https://github.com/Soleone/yap/commit/3e540a1f4fa7c1ed44dda78fdeaa35c02c8d47e9))
* speak text through a resident engine and the yap command ([e5a4e96](https://github.com/Soleone/yap/commit/e5a4e961e4ed674a23221668cfc35eb1e86ce1fa))
* status popup with subtitles, pause and stop ([3c91d6d](https://github.com/Soleone/yap/commit/3c91d6def53067a00dbdc6bb349eac7f87f1d365))
* Windows installer with the yap command on PATH, a speech bubble icon, startup checkbox ([3a2f844](https://github.com/Soleone/yap/commit/3a2f84416a461bc0632f2ec840f4524f3211d964))
