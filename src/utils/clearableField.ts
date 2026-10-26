/**
 * Choose an update payload value for an optional field. Returns the form's
 * value when it is set, `null` to clear a field the record held when the form
 * opened, and `undefined` to leave out a field that was empty then and is
 * empty now. Only `undefined`, `null`, and the empty string count as empty, so
 * a value such as `0` is sent as is.
 */
export function clearableField<T>(
  value: T | '' | undefined,
  loaded: unknown,
): T | null | undefined {
  if (!isEmpty(value)) return value as T
  return isEmpty(loaded) ? undefined : null
}

function isEmpty(value: unknown): boolean {
  return value === undefined || value === null || value === ''
}
