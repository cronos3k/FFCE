// Field-overlay colors and canvas drawing.
// Colors and blend math mirror ui.py (_algo_palette, _field_color_variant,
// _core_base_color, _draw_field_overlay, _draw_histogram).

export const ALGO_PALETTE = [
  [220, 80, 80], [80, 200, 120], [80, 140, 220], [220, 200, 80],
  [180, 80, 200], [80, 200, 200], [200, 120, 80], [120, 200, 80],
  [200, 80, 120], [120, 120, 220], [200, 200, 200], [150, 100, 60],
  [100, 160, 200],
];

export const CORE_COLORS = {
  core_pressure: [220, 80, 80],
  core_attack: [90, 180, 130],
  core_resistance: [90, 120, 200],
  core_trace: [210, 180, 80],
  core_flow: [180, 90, 200],
};

// Overlay UI mode -> base color used for the heatmap (per-field colors).
// mode: 1 pressure, 2 resistance, 3 trace, 4 attack, 5 extras-sum,
//       6 selected-extra, 7 flow.
export function overlayBaseColor(mode, selectedExtra) {
  switch (mode) {
    case 1: return CORE_COLORS.core_pressure;
    case 2: return CORE_COLORS.core_resistance;
    case 3: return CORE_COLORS.core_trace;
    case 4: return CORE_COLORS.core_attack;
    case 5: return [200, 200, 200];
    case 6: return ALGO_PALETTE[selectedExtra % ALGO_PALETTE.length];
    case 7: return CORE_COLORS.core_flow;
    default: return [200, 200, 200];
  }
}

// Port of _field_color_variant: positive -> base, negative -> rotated base.
function fieldColorVariant(value, vmax, base) {
  if (vmax <= 1e-6) return null;
  let v = Math.max(-vmax, Math.min(vmax, value)) / vmax;
  const alpha = Math.min(1, (200 * Math.abs(v)) / 255);
  const color = v >= 0 ? base : [base[2], base[0], base[1]];
  return `rgba(${color[0]},${color[1]},${color[2]},${alpha.toFixed(3)})`;
}

function maxAbs(grid) {
  let m = 0;
  for (let i = 0; i < grid.length; i++) m = Math.max(m, Math.abs(grid[i]));
  return m + 1e-6;
}

// Draw a single field heatmap. `grid` is Float32Array(64) indexed y*8+x.
// `toScreen(x,y)` -> {r,c}. `sq` is the cell pixel size.
export function drawHeatmap(g2d, grid, base, sq, toScreen) {
  const vmax = maxAbs(grid);
  for (let y = 0; y < 8; y++) {
    for (let x = 0; x < 8; x++) {
      const color = fieldColorVariant(grid[y * 8 + x], vmax, base);
      if (!color) continue;
      const { r, c } = toScreen(x, y);
      g2d.fillStyle = color;
      g2d.fillRect(c * sq, r * sq, sq, sq);
    }
  }
}

// Draw the contributions histogram: per enabled extra, a horizontal bar per
// cell whose length is proportional to |field value| / max, in algo color.
// `entries` = [{ idx, field(Float32Array), color([r,g,b]) }], highlightIdx.
export function drawHistogram(g2d, entries, sq, toScreen, highlightIdx) {
  if (entries.length === 0) return;
  const lineH = Math.max(2, Math.floor(sq / (entries.length + 2)));
  const pad = 3;
  const width = sq - pad * 2;
  const vmaxByIdx = entries.map((e) => maxAbs(e.field));
  for (let y = 0; y < 8; y++) {
    for (let x = 0; x < 8; x++) {
      const { r, c } = toScreen(x, y);
      const baseX = c * sq + pad;
      const baseY = r * sq + pad;
      entries.forEach((e, k) => {
        const value = e.field[y * 8 + x];
        const vmax = vmaxByIdx[k];
        const length = Math.floor(width * Math.min(1, Math.abs(value) / vmax));
        if (length <= 0) return;
        let col = e.color.slice();
        if (value < 0) col = col.map((v) => Math.max(0, v - 40));
        if (e.idx === highlightIdx) col = col.map((v) => Math.min(255, v + 40));
        g2d.strokeStyle = `rgb(${col[0]},${col[1]},${col[2]})`;
        g2d.lineWidth = 2;
        const y0 = baseY + k * lineH + 1;
        g2d.beginPath();
        g2d.moveTo(baseX, y0);
        g2d.lineTo(baseX + length, y0);
        g2d.stroke();
      });
    }
  }
}
