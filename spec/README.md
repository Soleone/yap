# yap specification

Yap reads text aloud with a local voice model, starting within a fraction of a second, and keeps everything it read so it replays from disk. It is Handy in reverse: a tray app with a `yap` command, an optional hotkey, a small status popup with subtitles, and a library of past reads.

This spec says what the app does. It doesn't say how it is built, what is proven where, or why every decision was made; that lives in docs/.

## Reading a rule

Behavior is written as rules in tables:

| Rule         | When                              | Then                   |
| ------------ | --------------------------------- | ---------------------- |
| `theme.open` | `T` anywhere outside a text field | The theme picker opens |

- **Rule** is a stable slug, `area.behavior`. Tests put it in their title (`test("theme.open: ...")`).
- **When** is the situation or input. **Then** is the observable result, in plain present tense.
- **MUST** appears only where a rule could be misread as optional.
- A `> Why:` line follows a rule only when the reason isn't obvious.

Split into files by area (`spec/home.md`, `spec/settings.md`) once this one gets long, and link them here.

## Speaking

| Rule             | When                                                                                            | Then                                                                                                                                                                                            |
| ---------------- | ----------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `speak.args`     | `yap "text"`                                                                                    | The text is read aloud; the command returns once it finished playing                                                                                                                            |
| `speak.stdin`    | `yap` with text on stdin and no text arguments                                                  | Stdin is read aloud the same way                                                                                                                                                                |
| `speak.dashes`   | `yap -- "-5 degrees"`                                                                           | Everything after `--` is text, never a flag                                                                                                                                                     |
| `speak.voice`    | `--voice <name>`                                                                                | That voice reads, instead of the one in settings                                                                                                                                                |
| `speak.speed`    | `--speed <multiplier>`                                                                          | That speed reads, rounded to the closest one the model offers                                                                                                                                   |
| `speak.detect`   | A read starts with other languages set up                                                       | Its language is told from its whole text, among the main language and the others, and it reads in that language's model and voice. Text too short or unclear to tell reads in the main language |
| `speak.language` | `--language <code>`                                                                             | That language's model and voice read, without telling it from the text; a language not set up reads in the main model and voice                                                                 |
| `speak.source`   | `--app`, `--title`, `--url`, or `--source-env <prefix>` for `<prefix>_APP`, `_TITLE` and `_URL` | Where the text came from is kept with the read in the library                                                                                                                                   |
| `speak.replace`  | A read starts while another plays                                                               | The earlier one stops and its command returns                                                                                                                                                   |
| `speak.stop`     | `yap --stop`, or the reading command is killed                                                  | Playback stops at once                                                                                                                                                                          |
| `speak.pause`    | `yap --pause`                                                                                   | Playback pauses, or resumes if paused                                                                                                                                                           |
| `speak.save`     | `yap --save "text"`                                                                             | The read is recorded in the library and made in the background, in the saved-reads model and voice, or its language's own when it has one, without playing; the command returns at once         |
| `speak.open`     | `yap` in a terminal with no text                                                                | The window opens                                                                                                                                                                                |
| `speak.launch`   | Any `yap` command while the app isn't running                                                   | The app starts in the tray, then handles the command. The app MUST NOT hold the command's output open                                                                                           |
| `speak.source`   | `--app`, `--title` or `--url`                                                                   | The read records where its text came from                                                                                                                                                       |
| `speak.instant`  | A read starts with the model loaded                                                             | Audio starts within a few hundred milliseconds                                                                                                                                                  |

> Why `speak.launch` says MUST: Pluck waits for the command's output to end, so an app holding it would never let a read finish.

## Text

| Rule          | When                                               | Then                                                                              |
| ------------- | -------------------------------------------------- | --------------------------------------------------------------------------------- |
| `text.tables` | The text holds a markdown table                    | Each row is read as "Header: value, Header: value."                               |
| `text.code`   | The text holds a fenced code block                 | "A code block, skipped." is read instead                                          |
| `text.markup` | Headings, list marks, emphasis, inline code, links | The marks and link targets are not read; headings and list items end with a pause |
| `text.pieces` | Any text                                           | It is read as sentences; sentences under 40 characters join the next              |

## Library

| Rule              | When                                                          | Then                                                                                                                                                                                                                                                                                                            |
| ----------------- | ------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `library.record`  | A read starts                                                 | It is recorded with its text, first words as a title, source, voice and date. The same text read again moves to the top instead of showing twice                                                                                                                                                                |
| `library.replay`  | A sentence was read before in the same model, voice and speed | It plays from disk, with quote styles, dashes and other punctuation that doesn't change the sound ignored                                                                                                                                                                                                       |
| `library.partial` | A read stops mid-sentence                                     | That sentence is not stored                                                                                                                                                                                                                                                                                     |
| `library.list`    | The library opens                                             | Reads show newest first, with where they came from, the voice and when                                                                                                                                                                                                                                          |
| `library.play`    | Play on a read, or moving its seek bar                        | It plays again from that sentence, in the voice, model and speed it was read in                                                                                                                                                                                                                                 |
| `library.delete`  | Delete on a read                                              | It goes away, with the stored sentences no other read uses                                                                                                                                                                                                                                                      |
| `library.save`    | Reads saved with `--save`                                     | They are made one at a time, oldest first, apart from reading, so a read starts at once while they are made. The library shows how many sentences are ready, and play waits until all are. A save cut off by quitting carries on at the next start; a failed one shows why and is tried again at the next start |
| `library.keep`    | Keep on a read                                                | It is never pruned until kept is turned off again                                                                                                                                                                                                                                                               |
| `library.prune`   | A read wasn't played for the days set in Keep reads           | It is deleted like `library.delete`, unless it is kept or still being saved. A saved read counts from when it was finished                                                                                                                                                                                      |

## Popup

| Rule              | When                                        | Then                                                                                           |
| ----------------- | ------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `popup.show`      | A read plays and the popup style isn't none | A popup shows at the top or bottom center of the screen under the cursor, without taking focus |
| `popup.subtitles` | Style subtitles                             | The current sentence shows, with the spoken words highlighted as they go                       |
| `popup.minimal`   | Style minimal                               | The source, voice and progress show                                                            |
| `popup.off`       | Style no popup                              | Nothing shows while reading                                                                    |
| `popup.controls`  | Hovering the popup                          | A pause button and an x show; the x stops the read and hides the popup                         |

## Settings

| Rule                 | When                                                   | Then                                                                                                                                                                                                                                                          |
| -------------------- | ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `settings.voice`     | Choosing a voice                                       | The model's voices list with their descriptions, and a preview line plays                                                                                                                                                                                     |
| `settings.speed`     | Moving the speed slider                                | Its steps are the ones the model offers; the next read uses it                                                                                                                                                                                                |
| `settings.language`  | Choosing a language                                    | Only the models that read it list; the current model stays when it reads it, else the first that does                                                                                                                                                         |
| `settings.languages` | Adding another language                                | It lists under Other languages with a model and voice of its own, which load next to the main ones; choosing it as the main language takes it out of the others                                                                                               |
| `settings.model`     | Choosing a model                                       | Each lists with its download size, its Python runtime included; its license, its other languages when it has several, and whether it needs an NVIDIA GPU show below. Choosing one downloads it, and a voice it has replaces one it doesn't                    |
| `settings.token`     | Choosing a gated model                                 | A field for a Hugging Face token shows; the token is saved apart from the settings and never shown again. A model that fails to load says why in a line, a gated one which terms to accept, and Try again loads it again                                      |
| `settings.save`      | Choosing a model for saved reads                       | `--save` uses it and its voice; "Same as reading" uses the reading model and voice                                                                                                                                                                            |
| `settings.history`   | Choosing how long to keep reads                        | 7, 30 (the default), 90 or 365 days since the last play, or forever                                                                                                                                                                                           |
| `settings.folder`    | Choosing a models folder in the system's folder dialog | Models and their Python runtimes download there from the next start, which a restart button offers; ones already downloaded stay where they were. The default is the data folder                                                                              |
| `settings.hotkey`    | Pressing the hotkey                                    | The selected text of the app in front is read, with that app, window title and page as its source; the clipboard is put back. With nothing selected, it stops reading. Off until set; on Wayland the window manager binds `wl-paste --primary \| yap` instead |
| `settings.startup`   | Start with the system                                  | Yap starts in the tray when you log in                                                                                                                                                                                                                        |
| `settings.version`   | Opening settings                                       | The app's version shows at the bottom                                                                                                                                                                                                                         |

## Install

| Rule                | When                                                                         | Then                                                                                                                                                               |
| ------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `install.command`   | Installing on Windows                                                        | One installer puts the app and the `yap` command in place and adds that folder to the user's PATH, so new terminals and Pluck find it                              |
| `install.uninstall` | Uninstalling                                                                 | The folder leaves PATH; models, pieces, the library and settings stay unless "Delete the application data" is checked                                              |
| `install.python`    | Choosing a model that runs in Python                                         | uv, which must be installed, downloads Python, the model's packages and its weights into the models folder the first time; without uv the model says how to get it |
| `install.update`    | A newer release is on GitHub when the Yap window opens or comes to the front | A banner offers it; Update downloads, installs and restarts yap, and a failure offers Retry. Development builds never check                                        |

## Glossary

| Term     | Meaning                                                        |
| -------- | -------------------------------------------------------------- |
| read     | One text handed to yap, from start to end                      |
| piece    | One sentence of a read, generated and stored as one audio file |
| registry | The list of models yap can download                            |
