# Director Answers Update — 2026-10-04 (22:58)

**From:** Director (Joe)  
**Distributed by:** VaultKeeper  
**Target:** 1A Builder (`WrzDJ-worktrees/kiosk-1a`), Reviewer Claude (session 2), Codex Preview  
**Source of Truth:** `WrzDJ-vault/director-answers/2026-10-04-answers-batch-1.md`  

---

## 1. Locked Decisions for 1A Implementation

### Theme (Q2)
- **Selection:** **Electric Violet with Liquid Metal** (`#a855f7` accent).
- **Rule:** No theme picker or controls on the kiosk. Monochrome Studio and Obsidian Amber are parked as future configuration options in the DJ dashboard settings, keeping the public display clean and true to the original.

### Recently Played Placement (Q3)
- **Selection:** **Option 2b** — compact strip of the last 3 tracks placed inside the queue area beneath requests.
- **Visuals:** Focus matching the `kiosk-preview-1920` and `1280` layout styles.
- **Future:** Option 2d (dedicated 3rd column) and alternative layouts are parked as future selectable layout options in DJ dashboard settings.

### Hero Split (Q4)
- **Selection:** **65% / 35%** (65% artwork + track metadata, 35% QR request code).

### Vote-to-Song Binding (Q5)
- **Selection:** 4A "keep marked no song" + 4B "snapshot per vote".
- **Default:** Reject votes when no song is playing by default; configurable per-event to allow "no song" votes with greyed-out meter.

### Buyer Target (Q6)
- **Selection:** Single DJ first. Maintain flexible data model so venue/promoter features can follow later.

### 1A Design Approval (Q7)
- **Selection:** **YES** — proceed with building 1A with Recently Played (2b) included.

### Parked Components
- `HairlineTurntable.tsx`: Parked on kiosk; candidate for search request loading animation later.
- `NewcomerGuideModal.tsx`: Parked on kiosk (no dead controls/modals on public display).

---

--VaultKeeper
