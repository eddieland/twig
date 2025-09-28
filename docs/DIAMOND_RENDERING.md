# Diamond Pattern Rendering in Twig

Twig's Component 8 introduces advanced diamond pattern rendering capabilities for visualizing complex git branch structures with merge patterns, cross-references, and deep nesting support.

## Overview

Diamond patterns occur when two or more branches diverge from a common ancestor and later merge together, forming a "diamond" shape in the branch tree. This is common in git workflows where feature branches are merged back into main branches.

## Features

### 8.1 Diamond Pattern Detection
- **Automatic Detection**: Identifies diamond patterns in complex git branch structures
- **Nested Support**: Handles nested diamonds (diamonds within diamonds)
- **Path Analysis**: Analyzes merge paths and identifies common ancestors
- **Cycle Detection**: Detects circular dependencies in branch relationships

### 8.2 Enhanced Tree Visualization
- **Unicode Symbols**: Beautiful diamond symbols for different pattern roles
  - `◇` - Diamond ancestor (divergence point)
  - `◆` - Diamond merge point (convergence point) 
  - `◊` - Diamond branch (intermediate branch in pattern)
  - `◈` - Complex diamond (multiple merge points)
- **ASCII Fallback**: Automatic fallback to ASCII characters for compatibility
- **Color Support**: Colored output with no-color mode support

### 8.3 Cross-Reference Handling
- **Circular Dependencies**: Detects and visualizes circular branch dependencies
- **Reference Counting**: Shows how many times branches are referenced
- **Visual Indicators**: Special symbols for revisited branches (🔄, ↑)
- **"See Above" References**: Clear indicators when branches appear multiple times

### 8.4 Deep Nesting Support
- **Pagination**: Handles large numbers of child branches with page navigation
- **Pruning**: Intelligent pruning of very deep or wide trees
- **Memory Optimization**: Memory usage estimation and optimization
- **Performance Tracking**: Statistics for rendering performance
- **Navigation Aids**: Depth indicators and child count summaries

## Usage Examples

### Basic Diamond Rendering

```rust
use twig_core::tree_renderer::TreeRenderer;

// Create renderer with branch data
let mut renderer = TreeRenderer::new(&branches, &roots, None, false);

// Render with diamond visualization
let mut output = Vec::new();
renderer.render_with_diamonds(&mut output, &roots, None, true)?;
```

### Enhanced Cross-Reference Rendering

```rust
// Render with cross-reference detection
let mut output = Vec::new();
renderer.render_with_enhanced_cross_refs(
    &mut output, 
    &roots, 
    None,           // delimiter
    true,           // show_cross_refs
    Some(10)        // max_ref_depth
)?;
```

### Deep Nesting with Configuration

```rust
use twig_core::tree_renderer::{DeepNestingConfig, RenderStats};

let config = DeepNestingConfig {
    max_depth: Some(20),              // Limit depth to 20 levels
    max_branches_per_level: Some(50), // Max 50 branches per level
    enable_pagination: true,          // Enable pagination for large lists
    page_size: 10,                    // 10 items per page
    enable_pruning: true,             // Enable intelligent pruning
    prune_threshold: 100,             // Prune trees with >100 branches
    show_depth_indicators: true,      // Show [depth:N] indicators
};

let mut output = Vec::new();
let stats = renderer.render_with_deep_nesting(&mut output, &roots, &config)?;

println!("Rendered {} branches, max depth: {}", 
         stats.total_branches, stats.max_depth_reached);
```

## Configuration Options

### DeepNestingConfig

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `max_depth` | `Option<u32>` | `Some(20)` | Maximum tree depth before truncation |
| `max_branches_per_level` | `Option<usize>` | `Some(50)` | Maximum branches per level before pruning |
| `enable_pagination` | `bool` | `false` | Enable pagination for large branch lists |
| `page_size` | `usize` | `10` | Number of branches per page |
| `enable_pruning` | `bool` | `false` | Enable intelligent subtree pruning |
| `prune_threshold` | `usize` | `100` | Total branches threshold for pruning |
| `show_depth_indicators` | `bool` | `true` | Show depth information in output |

### RenderStats

| Field | Type | Description |
|-------|------|-------------|
| `total_branches` | `usize` | Total number of branches in tree |
| `max_depth_reached` | `u32` | Maximum depth achieved during rendering |
| `branches_pruned` | `usize` | Number of branches pruned for performance |
| `circular_deps_detected` | `usize` | Number of circular dependencies found |
| `memory_usage_estimate` | `usize` | Estimated memory usage in bytes |

## Performance Considerations

### Large Repositories

For repositories with many branches (100+), consider:

1. **Enable Pruning**: Use `enable_pruning: true` to handle very large trees
2. **Set Depth Limits**: Use `max_depth` to prevent excessive nesting
3. **Use Pagination**: Enable pagination for branches with many children
4. **Monitor Memory**: Check `memory_usage_estimate` in render statistics

### Unicode Support

- Unicode symbols require terminal support for proper display
- ASCII fallback is automatically used when `no_color` mode is enabled
- Test unicode rendering in your terminal before enabling for production

### Circular Dependencies

- Circular dependency detection has O(V + E) complexity
- Large circular chains may impact performance
- Consider setting `max_ref_depth` to limit deep reference checking

## Visual Output Examples

### Simple Diamond Pattern
```
main [ancestor]
├── feature1
├── feature2  
└◆─ merge [merge]                    [via: feature1, feature2]
    └── post-merge
```

### Deep Nesting with Pagination
```
main
├── feature [depth:1] [children:25]
│   ├── sub1 [depth:2]
│   ├── sub2 [depth:2]
│   ├── ... Page 2 (11-20 of 25) ...
│   ├── sub11 [depth:2]
│   └── sub20 [depth:2]
```

### Circular Dependencies
```
⚠️  2 circular dependencies detected
  Cycle 1: branch1 → branch2 → branch1
  Cycle 2: feature → hotfix → main → feature

main
├🔄─ feature [CIRCULAR]
└── hotfix ↑ (see above)
```

## Best Practices

1. **Progressive Enhancement**: Start with basic rendering, add diamond features as needed
2. **Performance Testing**: Test with representative repository sizes
3. **Configuration Tuning**: Adjust thresholds based on typical repository structure
4. **Terminal Compatibility**: Test unicode rendering across target environments
5. **Memory Monitoring**: Monitor memory usage for very large repositories

## Error Handling

The diamond rendering system gracefully handles:
- Missing branch nodes (skipped silently)
- Circular references (detected and marked)
- Memory constraints (pruning activated)
- Terminal limitations (ASCII fallback)
- Invalid configurations (sensible defaults used)

## Integration with Twig CLI

Diamond rendering is integrated into twig's tree visualization commands:

```bash
# Enable diamond pattern detection
twig tree --diamonds

# Enhanced cross-reference handling  
twig tree --cross-refs --max-depth 10

# Deep nesting with pagination
twig tree --deep-nesting --page-size 15 --max-depth 25
```

See the main twig documentation for complete CLI usage examples.