# Database and Scanning Performance Optimizations

## Overview
This document outlines the performance optimizations implemented to improve library scanning performance, especially when dealing with large music collections.

## Database Schema Optimizations

### Added Indexes
The migration now includes several strategic indexes to optimize common queries:

1. **`idx_track_modified`** - Index on `modified` timestamp
   - Enables fast filtering of recently modified files
   - Critical for incremental scanning performance

2. **`idx_track_path`** - Index on `path` column
   - Enables fast lookups during scanning to check if files exist
   - Complements the existing unique constraint

3. **`idx_track_artist_album`** - Composite index on `artist` + `album`
   - Optimizes common music library queries
   - Enables fast album/artist browsing

4. **`idx_track_album_artist`** - Index on `album_artist`
   - Optimizes album-level grouping and browsing
   - Handles compilation albums efficiently

5. **`idx_track_album`** - Index on `album`
   - Enables fast album-based queries and grouping

6. **`idx_track_genre`** - Index on `genre`
   - Enables fast genre-based filtering and browsing

7. **`idx_track_year`** - Index on `year`
   - Enables chronological queries and filtering

8. **`idx_track_album_disc_track`** - Composite index for track ordering
   - Optimizes track listing within albums
   - Enables proper disc and track number ordering

9. **`idx_track_extension`** - Index on file extension
   - Enables fast filtering by file type
   - Useful for format-specific queries

## Scanning Algorithm Optimizations

### New Optimized Scanning Mode
Implemented `scan_dir_optimized()` which addresses the major performance bottleneck of loading all tracks into memory.

#### Key Improvements:

1. **Batch Database Queries**
   - Instead of loading all tracks: `SELECT * FROM tracks` (expensive for large libraries)
   - Now uses batched queries: `SELECT path, modified FROM tracks WHERE path IN (batch_of_paths)`
   - Configurable batch size (default: 1000 paths per query)

2. **Memory Efficiency**
   - Old approach: Load entire database into HashMap (memory intensive)
   - New approach: Query only relevant paths in small batches
   - Dramatically reduces memory usage for large libraries

3. **Database Connection Handling**
   - Proper connection cloning for async tasks
   - Avoids borrowing issues in concurrent scanning

### Configuration Options
New `ScanConfig` options for performance tuning:

```rust
pub struct ScanConfig {
    pub music_path: String,
    pub show_progress: bool,
    pub batch_size: usize,           // Tracks per database upsert (default: 100)
    pub path_batch_size: usize,      // Paths per database query (default: 1000)
    pub use_optimized_scanning: bool, // Enable new optimized mode (default: true)
}
```

## Performance Impact

### Before Optimizations:
- **Memory Usage**: O(n) where n = total tracks in database
- **Database Queries**: 1 large query loading entire database + individual upserts
- **Scanning Time**: Linear increase with database size, even for unchanged files

### After Optimizations:
- **Memory Usage**: O(batch_size) - constant small memory footprint
- **Database Queries**: Efficient batched queries with indexed lookups
- **Scanning Time**: Near-constant time for unchanged files, regardless of database size

## Dependencies Added

For the enhanced progress bar functionality, the following dependencies were added:

```toml
indicatif = "0.17.8"              # Modern progress bar library
indicatif-log-bridge = "0.2.2"    # Log integration with progress bars
```

**Removed:**
```toml
ml-progress = "0.1.0"  # Replaced with indicatif
```

## Usage

### Migration
Run the migration to recreate the database with indexes:
```bash
cargo run --bin migration
```

### Optimized Scanning
The optimized scanning is enabled by default. To customize:

```rust
let config = ScanConfig {
    use_optimized_scanning: true,
    path_batch_size: 1000,  // Adjust based on memory/performance trade-off
    batch_size: 100,        // Adjust based on database performance
    ..Default::default()
};
```

### Fallback Mode
The original scanning algorithm is preserved for compatibility:
```rust
let config = ScanConfig {
    use_optimized_scanning: false,  // Use original algorithm
    ..Default::default()
};
```

## Monitoring

The scanner now provides enhanced logging and more accurate progress reporting:
- **Progress bar updates after database operations**: Progress now reflects actual completed work (tracks upserted to database) rather than files scanned
- **Batch processing progress**: Tracks are processed and upserted in configurable batches
- **Database query performance**: Efficient batched queries with indexed lookups
- **Memory-efficient operation confirmation**: No large memory usage for existing track lookups

### Progress Bar Behavior
- **Before**: Progress updated after reading file metadata (misleading - work not actually complete)
- **After**: Progress updated after successful database upsert operations (accurate representation of completed work)
- **Display**: 
  - **Enhanced visual progress bar** with high-resolution Unicode characters (█▉▊▋▌▍▎▏)
  - **Comprehensive information**: elapsed time, progress bar, position/total, ETA, and processing rate
  - **Better positioning**: Progress bar outputs to stderr to avoid interfering with log messages
  - **Template**: `[00:01:23] [████████████████░░░░] 1,234/5,678 files processed (2m 15s @ 45.2/s)`
  - **Total estimation**: Based on file count (since not all files need processing)
  - **Accurate updates**: Progress increments only after tracks are successfully saved to database
  - **Smart completion**: Remaining files are marked as processed at the end for visual completion
- **Benefits**: True progress tracking while maintaining informative display with totals and rates

### Technical Improvements
- **Switched from `ml-progress` to `indicatif`**: More mature and feature-rich progress bar library
- **MultiProgress container**: Uses `MultiProgress` for better handling of concurrent progress bars and log output
- **Better terminal handling**: Progress bar doesn't interfere with logging output through proper cursor management
- **Enhanced visual feedback**: Higher quality progress bar with smooth animations and better formatting
- **Log bridge capability**: Added `indicatif-log-bridge` dependency for future enhanced log integration

## Expected Performance Improvements

For a library with 100,000 tracks:
- **Memory usage**: ~1GB → ~10MB (100x reduction)
- **Initial scan**: Minimal change (limited by file I/O)
- **Incremental scans**: 10-100x faster (mostly unchanged files skip processing)
- **Database responsiveness**: Maintained during scanning (no large table locks)

## Notes

- The optimized scanning is backward compatible
- All existing APIs remain unchanged
- Indexes will improve all music library queries, not just scanning
- The optimizations scale well with library size
- **Progress bar library**: Upgraded from `ml-progress` to `indicatif` for better terminal handling and positioning
- **Enhanced UX**: Progress bar now displays to stderr to avoid interfering with log output while providing rich visual feedback
