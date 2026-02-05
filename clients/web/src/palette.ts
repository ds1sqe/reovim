/**
 * Colorblind-friendly palette for multi-client presence visualization.
 *
 * Uses CBF-8 palette designed to be distinguishable for all types of
 * color vision deficiency (deuteranopia, protanopia, tritanopia).
 *
 * Based on Wong (2011) "Points of view: Color blindness", Nature Methods.
 *
 * @module palette
 */

/**
 * CBF-8: Colorblind-friendly palette with 8 distinct colors.
 *
 * Each color is distinguishable for all types of color vision deficiency.
 * Colors are ordered by perceptual distinctiveness.
 */
export const REMOTE_CLIENT_COLORS: readonly string[] = [
  "#E69F00", // Orange
  "#56B4E9", // Sky Blue
  "#009E73", // Bluish Green
  "#F0E442", // Yellow
  "#0072B2", // Blue
  "#D55E00", // Vermillion
  "#CC79A7", // Reddish Purple
  "#00C896", // Teal
] as const;

/**
 * Get the assigned color for a client ID.
 *
 * Colors are deterministically assigned based on `clientId % 8`.
 * This ensures the same client always gets the same color across sessions.
 *
 * @param clientId - The client's unique identifier
 * @returns Hex color string (e.g., "#E69F00")
 *
 * @example
 * ```typescript
 * // Client 0 gets orange
 * colorForClient(0n); // "#E69F00"
 *
 * // Client 8 wraps around to orange again
 * colorForClient(8n); // "#E69F00"
 * ```
 */
export function colorForClient(clientId: bigint): string {
  return REMOTE_CLIENT_COLORS[Number(clientId % 8n)];
}

/**
 * Get a dimmed version of a client's color (for selection backgrounds).
 *
 * Returns the same hue with reduced opacity, suitable for overlaying
 * on existing content without obscuring it.
 *
 * @param clientId - The client's unique identifier
 * @returns CSS rgba color string with low opacity
 *
 * @example
 * ```typescript
 * dimmedColorForClient(0n); // "rgba(230, 159, 0, 0.15)"
 * ```
 */
export function dimmedColorForClient(clientId: bigint): string {
  const hex = colorForClient(clientId);
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, 0.15)`;
}

/**
 * Get a darker version of a client's color (for cursor backgrounds).
 *
 * Returns the same hue with higher opacity, suitable for cursor
 * indicators that need to stand out.
 *
 * @param clientId - The client's unique identifier
 * @returns CSS rgba color string with moderate opacity
 */
export function darkColorForClient(clientId: bigint): string {
  const hex = colorForClient(clientId);
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, 0.85)`;
}
