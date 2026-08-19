import type { InjectionKey } from "vue";

/**
 * Provided by `SettingsField` so a slotted form control can associate itself
 * with the field's visible label:
 *
 * - `id`:      set on a labelable control (input/textarea) so the label's
 *              `for` attribute matches it.
 * - `labelId`: reference via `aria-labelledby` on non-labelable controls
 *              (e.g. a `UiSelect`'s button trigger).
 *
 * Base controls (`UiTextField`, `UiSelect`) consume this via `inject` and only
 * apply it when the consumer did not pass an explicit `id`/`aria-label`/
 * `aria-labelledby`. Composite fields (e.g. lists rendered with `v-for`)
 * should opt out with `no-association` to avoid duplicate ids.
 */
export interface FieldAssociation {
  id: string;
  labelId: string;
}

export const FIELD_ASSOCIATION: InjectionKey<FieldAssociation> = Symbol("field-association");
