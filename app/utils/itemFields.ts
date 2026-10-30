import type { CardField, IdentityField, ItemDraft } from '~/types/vault'

export const cardFields: { key: CardField; label: string }[] = [{ key: 'cardholderName', label: 'Cardholder name' }, { key: 'number', label: 'Card number' }, { key: 'code', label: 'Security code' }, { key: 'brand', label: 'Brand' }, { key: 'expMonth', label: 'Expiry month' }, { key: 'expYear', label: 'Expiry year' }]
export const identityFields: { key: IdentityField; label: string }[] = [{ key: 'title', label: 'Title' }, { key: 'firstName', label: 'First name' }, { key: 'middleName', label: 'Middle name' }, { key: 'lastName', label: 'Last name' }, { key: 'username', label: 'Username' }, { key: 'email', label: 'Email' }, { key: 'phone', label: 'Phone' }, { key: 'company', label: 'Company' }, { key: 'address1', label: 'Address' }, { key: 'address2', label: 'Address line 2' }, { key: 'address3', label: 'Address line 3' }, { key: 'city', label: 'City' }, { key: 'state', label: 'State' }, { key: 'postalCode', label: 'Postal code' }, { key: 'country', label: 'Country' }, { key: 'ssn', label: 'National identity number' }, { key: 'passportNumber', label: 'Passport number' }, { key: 'licenseNumber', label: 'License number' }]

// Best effort: JavaScript/DOM/IPC copies cannot promise allocator-level erasure.
// Callers keep drafts in component refs only and discard them on close/lock.
export function eraseItemDraft(value: ItemDraft | null) {
  if (!value) return
  value.name = ''; value.notes = ''; value.username = ''; value.password = ''; value.totp = ''; value.uri = ''
  for (const field of cardFields) if (value.card) value.card[field.key] = ''
  for (const field of identityFields) if (value.identity) value.identity[field.key] = ''
}
