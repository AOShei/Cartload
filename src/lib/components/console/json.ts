/** One piece of highlighted JSON: its class (`key`, `string`, `number`, `literal`, `punct`,
 *  or `plain` for whitespace) and its text. */
export interface JsonToken {
  kind: "key" | "string" | "number" | "literal" | "punct" | "plain";
  text: string;
}

/* A string (with escapes), then a number, `true` / `false` / `null`, punctuation, and
   whitespace. Pretty-printed JSON has nothing else. */
const TOKEN = /("(?:\\.|[^"\\])*")(\s*:)?|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|\b(true|false|null)\b|([{}[\],:])|(\s+)/g;

/** Splits pretty-printed JSON into tokens for display. A string followed by `:` is a key. */
export function highlightJson(text: string): JsonToken[] {
  const tokens: JsonToken[] = [];
  let last = 0;
  for (const match of text.matchAll(TOKEN)) {
    const index = match.index ?? 0;
    if (index > last) {
      tokens.push({ kind: "plain", text: text.slice(last, index) });
    }
    const [whole, string, colon, number, literal, punct] = match;
    if (string !== undefined) {
      tokens.push({ kind: colon ? "key" : "string", text: string });
      if (colon) {
        tokens.push({ kind: "punct", text: colon });
      }
    } else if (number !== undefined) {
      tokens.push({ kind: "number", text: number });
    } else if (literal !== undefined) {
      tokens.push({ kind: "literal", text: literal });
    } else if (punct !== undefined) {
      tokens.push({ kind: "punct", text: punct });
    } else {
      tokens.push({ kind: "plain", text: whole });
    }
    last = index + whole.length;
  }
  if (last < text.length) {
    tokens.push({ kind: "plain", text: text.slice(last) });
  }
  return tokens;
}
