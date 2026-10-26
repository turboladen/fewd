/**
 * Choose an update payload value for an optional field. Returns the form's
 * value when it is set, `null` to clear a field the record held when the form
 * opened, and `undefined` to leave out a field that was empty then and is
 * empty now. An empty string counts as empty on both sides.
 */
export function clearableField<T>(
  value: T | '' | undefined,
  loaded: unknown,
): T | null | undefined {
  if (value !== undefined && value !== '') return value
  return loaded !== undefined && loaded !== null && loaded !== '' ? null : undefined
}
