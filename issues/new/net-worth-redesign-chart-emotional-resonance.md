# Add emotional resonance to Net Worth chart

The chart uses default ECharts styling with generic blue (#3b82f6) and treats all
data points with the same visual weight regardless of context. Net worth is emotional
- the visualization should reflect that.

## Current state

```typescript
areaStyle: {
    opacity: isDarkMode() ? 0.15 : 0.1,
},
lineStyle: {
    width: 2,
    color: "#3b82f6",
},
```

## Problems

1. Blue is the most common "safe" choice - no personality
2. Area fill is static regardless of trajectory
3. No visual landmarks at significant points (all-time highs, major changes)
4. The three series colors (blue, green, amber) have no relationship - they feel
   arbitrary rather than part of a cohesive palette

## Suggested improvements

1. Tint the area fill based on trend direction:
   - Upward trends: subtle green/positive tones
   - Flat/downward: neutral/muted tones
2. Add subtle visual markers at all-time high points
3. Use a more cohesive color palette where the three series feel like variations
   of a theme rather than random picks
4. Consider gradient fills that respond to the data (darker at peaks, lighter in
   valleys) to give the chart more depth

## Relevant files

- `static/ts/net-worth-chart.ts`
- `static/css/input.css` (for any CSS custom properties)
