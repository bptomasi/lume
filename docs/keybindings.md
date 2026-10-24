# Keyboard and mouse

`?` opens the full built-in reference. Type `:` to search commands and inspect
their descriptions; `Tab` completes a command without executing it. Reader
motions act on readable text and preserve Unicode grapheme boundaries.

| Context | Keys | Action |
| --- | --- | --- |
| Welcome | Any key; `Ctrl-c` | Enter the library; exit |
| Navigation | `j/k`, arrows, `gg/G`, `Ctrl-d/u` | Move, jump and page |
| Panels | `Tab`, `Shift-Tab`, `Ctrl-w` | Change focus; open the layout guide |
| Article | `Enter`, `Z`, `Esc` | Open Reader; fullscreen; return |
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
focused target; article deletion goes to recoverable Trash. In Trash, `dd`
permanently deletes the article and its notes; the first `d` shows the warning
and `Esc` cancels. `:trash clean` permanently empties Trash. Neither permanent
operation can be undone. Refreshes do not reimport purged publisher identities.
Note deletion retains the note undo contract. Check `?` for counted motions,
jump history, find commands, editor search, tags, study cards and variants.

Clicking an article only selects its preview. Enter opens the Reader without
marking it read. By default, automatic marking requires 90% cumulative
displayed publisher text and ten active Reader seconds. A jump to the end
does not count skipped text. Other panes, dialogs and lost terminal window
focus pause time. `m` toggles read state explicitly; `:mark-read` and
`:mark-unread` set it. Explicit changes override automation for the current
article visit. `:auto-read off` disables automation; `:settings` and
`[ui.auto_read]` configure its thresholds.

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
`Ctrl-w =` or `:layout-reset` restores Library, Articles and Reader in that
order, with standard sidebar/headline widths and Annotations collapsed. It
leaves fullscreen and zen, preserving your article, notes, theme and prose
widths. `Ctrl-w u` undoes the reset, including after reopening Lume.
`Ctrl-w h/j/k/l` focuses the pane to the left/below/above/right in the current
layout and stops at its edges. Outside the Reader, `h/l` and left/right arrows
also follow panel geometry. `Tab` and `Shift-Tab` cycle the saved panel order.
Collapsed panels are skipped; narrow terminals retain access to panels hidden
by the responsive layout. Reader `h/l` still moves the text cursor.
Use commands such as `:panel-focus reader`, `:panel-focus articles` followed by `:panel-close`,
`:panel-focus annotations`, `:panel-move left` and
`:panel-dock annotations reader below`. `:layout-undo` restores the preceding
arrangement. `:workspace save NAME` and `:workspace load NAME` manage profiles.
`:fullscreen-width 80` sets the centered fullscreen reading column.

Click a panel to focus it. Drag a divider to resize; drag its title/grip to
reorder or dock it. Close/reopen controls are present for every panel.
Divider movement respects the minimum size of both nested sections. Articles
retains at least 24 columns while the terminal can fit the expanded panes;
responsive suppression applies when the whole terminal is too small.
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

Automatic detection selects Sixel for Foot and the iTerm2 protocol for native
inspection in WezTerm/iTerm2. Inline iTerm2 images still use character previews.
Other terminals fall back to blocks; the Kitty graphics protocol is not
integrated. `:images on` enables loading, `:images` inspects failures and
`:images retry` retries them. `:image-protocol auto` restores automatic choice.

Publisher `srcset` and lazy `data-srcset` images prefer the highest valid
candidate up to 4096 pixels wide or 4× density, falling back to `data-src`/`src`.
Downloads and decoded pixels retain their existing limits. Zoom and pan reuse
one decoded raster (at most 32 MiB); SVGs render again at the requested size.
Native inspection scales from the source with Lanczos filtering. Character
previews remain limited by terminal cells; they cannot reproduce pixel detail.

## Linked pages, offline reading and sessions

`Enter` on a reader link opens a temporary page inside Lume. Same-page links
jump directly to their section. `Ctrl-o` goes back, `Ctrl-i` goes forward, and
`Esc` returns to the original article. Linked readings preserve that article's
cursor and personal data. They are text views with explicit image links;
`o` opens the original in a browser. `:open-page URL` opens an address directly.
`:open-link` follows the cursor just like `Enter`, including relative links and
local sections. Missing sections on external pages show the page start with a
notice; opening the original preserves the requested fragment.
`:save-page` explicitly adds it to **Saved pages**, without subscribing to a
site. Saving an existing article preserves its identity and publisher text.
PDFs, other non-HTML responses and explicit download attachments open through
the system browser instead. HTML with unsupported text encoding also opens in
the browser. The reader does not decode non-HTML resource bodies;
redirect targets and fragments such as `#page=7` are preserved. The current
reading and cursor remain intact. Cancelled or superseded loads never launch
the browser. `:save-page` waits until the new linked page is ready.

`:offline queue` prepares **Read next**. Inside a personal folder,
`:offline prepare` captures that folder instead. Both download full text and
original images on a background lane. The optional final argument sets the
package budget in MiB, for example `:offline queue 128`. Defaults are 100
articles and 64 MiB; the byte budget accepts 1–512 MiB. Inspect
`:offline status`, resume with `:offline resume`, or request cancellation with
`:offline cancel`. Cancellation waits for the current bounded HTTP request.
A partial entry is never reported as fully ready. `:offline release` removes
pinned images and the checkpoint while keeping cached article text. Launch
`lume --offline` to read without HTTP.

`:session 20` previews a diverse sequence within a total estimate of 20 minutes.
It uses the current cached unread recommendations, so configure interests first.
`Enter` confirms the sequence and another `Enter` opens its current article.
`:session status` resumes it, including after restart; `:session next` advances
and skips removed or changed targets. `:session end` releases the sequence.
Planning and advancing do not mark articles read or imply positive feedback.

`:storage` reports logical text sizes, SQLite reusable pages, WAL and image
files. `:storage clean images|articles|all` previews an explicit cleanup;
`Enter` confirms. Feed text and personal records remain. Full caches attached
to favorites, queued articles, folders, notes/highlights, recoverable drafts,
the offline package or the remaining session are protected. Image cleanup
removes only ordinary
orphaned cache files. Completed cleanup batches remain committed after a later
failure or cancellation. `:storage cancel` requests cancellation between
batches. SQLite can reuse pages without shrinking its file.

`:performance on` enables bounded in-memory processing samples. Use
`:performance status` for counts, p50/p95/p99/max in milliseconds and pending
work; `off` stops sampling and `reset` clears it. Frame times include terminal
flush, but exclude compositor presentation. These are not physical key-to-pixel
measurements. Diagnostics send no telemetry and are disabled by default.

The field guide (`?`) has its own live search: `/` opens the query, Enter
keeps it, and `n` / `N` cycle matching rows. All query words must occur in
the same row. Esc cancels an edit, clears a kept query, then closes the guide.
This never filters the library or the article. Article search publishes matching
headlines independently of Reader preparation; Enter loads the chosen result.

Inline `$LaTeX$` and display `$$LaTeX$$` use a bounded Unicode projection,
including fractions, roots and super/subscripts. Formulas remain searchable,
selectable and anchored to their original source. This is terminal text rather
than TeX page typesetting; unsupported or excessive expressions stay verbatim.

Visible images use two download lanes and one shared decoder with a 32 MiB
source cache. Sixel clipping uses one palette and continuous diffusion per image,
avoiding color differences between independently encoded terminal rows.

`:latex-formulas on|off|toggle` saves `[ui] latex_formulas` (default `true`).
The same preference appears in `:settings`, including preview/cancel behavior.
Disabling keeps original delimiters and source; toggling retains source-based
cursor/selection positions and invalidates obsolete background layouts.
Unicode formulas include extended super/subscripts, sums/products and integral
bounds, function application, accents, binomial coefficients and compact matrix
rows. Missing script glyphs use grouped `^(...)` / `_(...)` notation. This is
portable terminal text, with unsupported layouts preserved as original LaTeX.
In graphics mode, text commands and linear inline expressions retain paragraph
flow; spatial notation and display equations can use local vector outlines.

In Library, `dd` sends every article in the selected built-in view to Trash:
Inbox, All articles, Read next, Favorites, Read, Notebook, Continue reading,
Since last visit, Grouped articles and For you. This acts on the full view,
including articles beyond the current page, without applying an Articles search.
Grouped articles includes all duplicate occurrences. Flags, personal folders
and notes remain attached; one `u` restores the complete batch. Permanently
deleting part of the batch keeps undo available for its surviving articles.
On Trash, `dd` permanently empties the entire Trash, just like `:trash clean`.
These are distinct from `dd` in Articles, which acts on one selected article.

On a feed/server, `dd` removes the source and its unsaved articles. On a smart
list or personal folder, it removes only the definition and keeps all articles.
The two presses must refer to the same destination; Esc, another action or
moving to another item cancels the pending removal. Autorepeat never confirms
it. Removing a server or using `:unfeed` also deletes its unsaved articles.
Favorites, Read next, personal folders, notes/highlights, tags, drafts,
exports and offline packages stay saved. Smart-list matches alone do not
protect articles; deleted entries disappear from all matching queries.
`:delete-smart` removes only the saved query and keeps its matching articles.
Personal-folder removal remains reversible with `u`.
Removing a server also removes its source references from smart lists. A list
whose entire source scope was removed stays named and editable but matches
nothing, rather than expanding into an unrestricted search.

In `:settings`, Enter edits the selected theme or numeric value and toggles
boolean switches. Enter applies a typed value to the live preview; Esc cancels
only that value edit. Ctrl-s saves the complete preview and closes Settings;
Esc outside a value editor discards the preview. Arrow adjustments still work.
