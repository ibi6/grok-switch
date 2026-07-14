/**
 * Mask a secret for UI/logs (mirrors Rust `core::mask::mask_secret`):
 * - length <= 12 → `***`
 * - `sk-` prefix → first 7 chars (`sk-` + 4) + `...` + last 4
 * - otherwise → first 6 + `...` + last 4
 */
export function maskSecret(secret: string): string {
  const s = secret.trim();
  const len = [...s].length;
  if (len <= 12) {
    return "***";
  }

  const chars = [...s];
  const prefix =
    s.startsWith("sk-") && len >= 7
      ? chars.slice(0, 7).join("")
      : chars.slice(0, 6).join("");
  const suffix = chars.slice(len - 4).join("");
  return `${prefix}...${suffix}`;
}
