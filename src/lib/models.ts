/** The console models the drawings and the Kernel page know, as the backend names them. */

export interface ConsoleModel {
  id: string;
  label: string;
}

export const CONSOLE_MODELS: readonly ConsoleModel[] = [
  { id: "SnesEur", label: "SNES Classic Mini (Europe)" },
  { id: "SnesUsa", label: "SNES Classic (North America)" },
  { id: "SuperFamicom", label: "Super Famicom Mini" },
  { id: "Nes", label: "NES Classic Mini" },
  { id: "Famicom", label: "Famicom Mini" },
  { id: "ShonenJump", label: "Famicom Mini (Shonen Jump edition)" },
];

export const DEFAULT_MODEL = "SnesEur";

export function isKnownModel(id: string | null | undefined): id is string {
  return typeof id === "string" && CONSOLE_MODELS.some((m) => m.id === id);
}

/** The model behind the USB product string of a console in normal boot on stock firmware
 *  (the stock kernel's gadget driver carries one string per model). Anything else, including
 *  no string at all (FEL mode), tells nothing. */
export function modelFromProduct(product: string | null | undefined): string | null {
  const text = (product ?? "").trim();
  switch (text) {
    case "CLV-S-NESY":
      return "Nes";
    case "CLV-S-HVCY":
      return "Famicom";
    case "CLV-S-SHVY":
      return "SuperFamicom";
    case "CLV-S-SNPY":
      return "SnesEur";
    case "CLV-S-SNSY":
      return "SnesUsa";
    default:
      return null;
  }
}
