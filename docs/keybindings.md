# Keyboard and mouse

`?` opens the full built-in reference. Type `:` to search commands and inspect
their descriptions; `Tab` completes a command without executing it. Reader
motions act on readable text and preserve Unicode grapheme boundaries.

| Context | Keys | Action |
| --- | --- | --- |
| Welcome | Any key; `Ctrl-c` | Enter the library; exit |
| Navigation | `j/k`, arrows, `gg/G`, `Ctrl-d/u` | Move, jump and page |
| Panels | `Tab`, `Shift-Tab`, `Ctrl-w` | Change focus; open the layout guide |
| Article | `Enter`, `Z`, `Esc` | Open and mark read; fullscreen; return |
| Search | `/`, `n/N` | Search the focused reader/list; next/previous match |
| Reader | `h/l`, `w/b/e`, `W/B/E` | Character, word and whitespace-delimited WORD motions |
| Reader | `0`, `^`, `$`, `I`, `A` | Row start, first text and row end |
| Reader | `gg/G`, `{`/`}`, `H/M/L` | Article, paragraph and viewport positions |
| Selection | `v`, `V`, `y`, `s` | Character/line selection, yank and highlight |
| Reader structure | `:toc`, `:copy-code`, `:tables` | Headings, code block copy and wide tables |
| Notes | `:note`, `:edit-note`, `dd` | Create, edit and delete a targeted note |
| Notebook | `Space n`, `Space /`, `Enter`, `Ctrl-o` | Open, search, follow source, return |
| Note editor | `Esc`, `:w`, `:wq`, `:wqa` | Normal mode, save and save/close |
| Organization | `:folder NAME`, `:smart`, `:mark-read` | Create a folder/list; mark an article read |
| Configuration | `:config`, `:config-reload`, `:settings` | Inspect, reload and preview appearance |

Commands requiring an article or selection enforce their context. Notes cannot
be created against a hidden article from an unrelated panel. `dd` uses the
focused target; article deletion goes to recoverable Trash, while note deletion
retains the note undo contract. Check `?` for counted motions, jump history,
find commands, editor search, tags, study cards and command variants.

## Tables and code

Markdown and normalized HTML tables render directly in the article with rounded
borders and left, center or right column alignment. `zh` / `zl` pan code and
tables by one terminal cell; `zH` / `zL` pan by half the reader width. Ordinary
cursor motions keep the selected cell visible. `:tables` opens a separate
inspector with `h/l` horizontal and `j/k` vertical navigation; `Esc` returns to
the same article position. Table text supports the usual search, selection,
yank and annotation keys.

Code blocks use a continuous background across the reading column, including
blank and short rows. Code fragments inside prose retain compact styling.

## Arrange your workspace

Press `Ctrl-w` to see the next available layout keys in the application.
Use commands such as `:panel-focus reader`, `:panel-focus articles` followed by `:panel-close`,
`:panel-focus annotations`, `:panel-move left` and
`:panel-dock annotations reader below`. `:layout-undo` restores the preceding
arrangement. `:workspace save NAME` and `:workspace load NAME` manage profiles.
`:fullscreen-width 80` sets the centered fullscreen reading column.

Click a panel to focus it. Drag a divider to resize; drag its title/grip to
reorder or dock it. Close/reopen controls are present for every panel.
Reader mouse selection stays inside the reader and uses source offsets rather
than terminal framebuffer text. Wheel events move one row/item each; a terminal
may emit several events per physical wheel step. Terminal-native selection can
require its mouse-reporting bypass modifier.

## Notes and studying

In the Notebook, use `e` to edit, `t` for tags, `T` for type, `p` to pin, `r`
to resolve/reopen, `dd` to delete and `u` to restore. `y` copies a note and `Y`
its captured quotation. Boolean note search supports article/source, tags,
type, status and dates. `:cards` reviews user-authored cards; `:review` opens
due notes. `:export-note-files DIRECTORY` reconciles incremental Markdown
exports, retaining local edits as conflicts.

## Images

Native inline rendering uses Foot Sixel strips. Other terminals receive a
portable character preview. Click an image to open the inspector, use `+/-`
to zoom, pan using the indicated controls and `Esc` to return. Selection,
menus and panels clip native rows within their owned rectangles. Clipboard
copy uses the terminal's OSC 52 support and follows terminal clipboard policy.
