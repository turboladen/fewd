import { useState } from 'react'
import { useCreatePerson, useDeletePerson, usePeople, useUpdatePerson } from '../hooks/usePeople'
import type { CreatePersonDto, UpdatePersonDto } from '../types/person'
import { parsePerson } from '../types/person'

function TagInput({
  label,
  value,
  onChange,
}: {
  label: string
  value: string[]
  onChange: (value: string[]) => void
}) {
  const [input, setInput] = useState('')

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && input.trim()) {
      e.preventDefault()
      onChange([...value, input.trim()])
      setInput('')
    }
  }

  const handleRemove = (index: number) => {
    onChange(value.filter((_, i) => i !== index))
  }

  return (
    <div>
      <label className='block text-sm font-medium text-gray-700 mb-1'>
        {label}
      </label>
      <div className='flex flex-wrap gap-1 mb-1'>
        {value.map((tag, i) => (
          <span
            key={i}
            className='inline-flex items-center bg-gray-100 text-gray-700 text-sm px-2 py-1 rounded'
          >
            {tag}
            <button
              onClick={() => handleRemove(i)}
              className='ml-1 text-gray-400 hover:text-gray-600'
              type='button'
            >
              x
            </button>
          </span>
        ))}
      </div>
      <input
        type='text'
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={`Add ${label.toLowerCase()} (press Enter)`}
        className='border border-gray-300 p-2 rounded w-full text-sm'
      />
    </div>
  )
}

interface PersonFormData {
  name: string
  birthdate: string
  dietary_goals: string
  dislikes: string[]
  favorites: string[]
  notes: string
}

const emptyForm: PersonFormData = {
  name: '',
  birthdate: '',
  dietary_goals: '',
  dislikes: [],
  favorites: [],
  notes: '',
}

function PersonForm({
  initialData,
  onSubmit,
  onCancel,
  submitLabel,
}: {
  initialData: PersonFormData
  onSubmit: (data: PersonFormData) => void
  onCancel: () => void
  submitLabel: string
}) {
  const [form, setForm] = useState<PersonFormData>(initialData)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit(form)
  }

  return (
    <form onSubmit={handleSubmit} className='space-y-3'>
      <div>
        <label className='block text-sm font-medium text-gray-700 mb-1'>
          Name
        </label>
        <input
          type='text'
          required
          value={form.name}
          onChange={(e) => setForm({ ...form, name: e.target.value })}
          className='border border-gray-300 p-2 rounded w-full'
        />
      </div>
      <div>
        <label className='block text-sm font-medium text-gray-700 mb-1'>
          Birthdate
        </label>
        <input
          type='date'
          required
          value={form.birthdate}
          onChange={(e) => setForm({ ...form, birthdate: e.target.value })}
          className='border border-gray-300 p-2 rounded w-full'
        />
      </div>
      <div>
        <label className='block text-sm font-medium text-gray-700 mb-1'>
          Dietary Goals
        </label>
        <input
          type='text'
          value={form.dietary_goals}
          onChange={(e) => setForm({ ...form, dietary_goals: e.target.value })}
          placeholder='e.g., More protein, less sugar'
          className='border border-gray-300 p-2 rounded w-full'
        />
      </div>
      <TagInput
        label='Favorites'
        value={form.favorites}
        onChange={(favorites) => setForm({ ...form, favorites })}
      />
      <TagInput
        label='Dislikes'
        value={form.dislikes}
        onChange={(dislikes) => setForm({ ...form, dislikes })}
      />
      <div>
        <label className='block text-sm font-medium text-gray-700 mb-1'>
          Notes
        </label>
        <textarea
          value={form.notes}
          onChange={(e) => setForm({ ...form, notes: e.target.value })}
          className='border border-gray-300 p-2 rounded w-full'
          rows={2}
        />
      </div>
      <div className='flex gap-2'>
        <button
          type='submit'
          className='bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700'
        >
          {submitLabel}
        </button>
        <button
          type='button'
          onClick={onCancel}
          className='border border-gray-300 px-4 py-2 rounded hover:bg-gray-50'
        >
          Cancel
        </button>
      </div>
    </form>
  )
}

export function FamilyManager() {
  const { data: people, isLoading, error } = usePeople()
  const createMutation = useCreatePerson()
  const updateMutation = useUpdatePerson()
  const deleteMutation = useDeletePerson()

  const [isAdding, setIsAdding] = useState(false)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<string | null>(null)

  const handleCreate = (formData: PersonFormData) => {
    const dto: CreatePersonDto = {
      name: formData.name,
      birthdate: formData.birthdate,
      dislikes: formData.dislikes,
      favorites: formData.favorites,
      dietary_goals: formData.dietary_goals || undefined,
      notes: formData.notes || undefined,
    }
    createMutation.mutate(dto, {
      onSuccess: () => setIsAdding(false),
    })
  }

  const handleUpdate = (id: string, formData: PersonFormData) => {
    const dto: UpdatePersonDto = {
      name: formData.name,
      birthdate: formData.birthdate,
      dislikes: formData.dislikes,
      favorites: formData.favorites,
      dietary_goals: formData.dietary_goals || undefined,
      notes: formData.notes || undefined,
    }
    updateMutation.mutate({ id, data: dto }, {
      onSuccess: () => setEditingId(null),
    })
  }

  const handleDelete = (id: string) => {
    deleteMutation.mutate(id, {
      onSuccess: () => setConfirmingDeleteId(null),
    })
  }

  if (isLoading) {
    return <div className='p-6 text-gray-500'>Loading family members...</div>
  }

  if (error) {
    return (
      <div className='p-6 text-red-600'>
        Failed to load family members: {String(error)}
      </div>
    )
  }

  return (
    <div className='p-6'>
      <div className='flex items-center justify-between mb-6'>
        <h1 className='text-2xl font-bold text-gray-900'>Family Members</h1>
        {!isAdding && (
          <button
            onClick={() => setIsAdding(true)}
            className='bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700'
          >
            + Add Person
          </button>
        )}
      </div>

      {isAdding && (
        <div className='mb-6 border border-gray-200 p-4 rounded-lg bg-white'>
          <h3 className='font-semibold text-lg mb-3'>Add Family Member</h3>
          <PersonForm
            initialData={emptyForm}
            onSubmit={handleCreate}
            onCancel={() => setIsAdding(false)}
            submitLabel='Add Person'
          />
          {createMutation.error && (
            <p className='mt-2 text-red-600 text-sm'>
              {String(createMutation.error)}
            </p>
          )}
        </div>
      )}

      {people?.length === 0 && !isAdding && (
        <p className='text-gray-500'>
          No family members yet. Add someone to get started!
        </p>
      )}

      <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
        {people?.map((person) => {
          const parsed = parsePerson(person)

          if (editingId === person.id) {
            return (
              <div
                key={person.id}
                className='border border-blue-200 p-4 rounded-lg bg-blue-50'
              >
                <h3 className='font-semibold text-lg mb-3'>
                  Edit {person.name}
                </h3>
                <PersonForm
                  initialData={{
                    name: person.name,
                    birthdate: person.birthdate,
                    dietary_goals: person.dietary_goals || '',
                    dislikes: parsed.dislikes,
                    favorites: parsed.favorites,
                    notes: person.notes || '',
                  }}
                  onSubmit={(data) => handleUpdate(person.id, data)}
                  onCancel={() => setEditingId(null)}
                  submitLabel='Save Changes'
                />
                {updateMutation.error && (
                  <p className='mt-2 text-red-600 text-sm'>
                    {String(updateMutation.error)}
                  </p>
                )}
              </div>
            )
          }

          return (
            <div
              key={person.id}
              className='border border-gray-200 p-4 rounded-lg bg-white'
            >
              <div className='flex items-start justify-between'>
                <h3 className='font-semibold text-lg'>{person.name}</h3>
                <div className='flex gap-2'>
                  <button
                    onClick={() => setEditingId(person.id)}
                    className='text-blue-600 text-sm hover:underline'
                  >
                    Edit
                  </button>
                  {confirmingDeleteId === person.id
                    ? (
                      <span className='flex gap-1 items-center text-sm'>
                        <span className='text-red-600'>Delete?</span>
                        <button
                          onClick={() => handleDelete(person.id)}
                          className='text-red-700 font-semibold hover:underline'
                        >
                          Yes
                        </button>
                        <button
                          onClick={() => setConfirmingDeleteId(null)}
                          className='text-gray-500 hover:underline'
                        >
                          No
                        </button>
                      </span>
                    )
                    : (
                      <button
                        onClick={() => setConfirmingDeleteId(person.id)}
                        className='text-red-600 text-sm hover:underline'
                      >
                        Delete
                      </button>
                    )}
                </div>
              </div>
              <p className='text-sm text-gray-500 mt-1'>
                Born: {person.birthdate}
              </p>
              {person.dietary_goals && (
                <p className='text-sm mt-1'>
                  <span className='font-medium'>Goals:</span> {person.dietary_goals}
                </p>
              )}
              {parsed.favorites.length > 0 && (
                <div className='mt-2'>
                  <span className='text-sm font-medium'>Favorites:</span>
                  <span className='text-sm text-gray-600'>
                    {parsed.favorites.join(', ')}
                  </span>
                </div>
              )}
              {parsed.dislikes.length > 0 && (
                <div className='mt-1'>
                  <span className='text-sm font-medium'>Dislikes:</span>
                  <span className='text-sm text-gray-600'>
                    {parsed.dislikes.join(', ')}
                  </span>
                </div>
              )}
              {person.notes && (
                <p className='text-sm text-gray-500 mt-2 italic'>
                  {person.notes}
                </p>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}
