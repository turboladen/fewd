import { useEffect, useState } from 'react'
import { useCreatePerson, useDeletePerson, usePeople, useUpdatePerson } from '../hooks/usePeople'
import type { CreatePersonDto, UpdatePersonDto } from '../types/person'
import { parsePerson } from '../types/person'
import { EmptyState } from './EmptyState'
import { IconEdit, IconPlus, IconTrash } from './Icon'
import { TagInput } from './TagInput'
import { useToast } from './Toast'

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

  const isValid = form.name.trim() !== '' && form.birthdate !== ''

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!isValid) return
    onSubmit(form)
  }

  return (
    <form onSubmit={handleSubmit} className='space-y-3'>
      <div>
        <label className='block text-sm font-medium text-stone-700 mb-1'>
          Name
        </label>
        <input
          type='text'
          required
          value={form.name}
          onChange={(e) => setForm({ ...form, name: e.target.value })}
          className='input w-full'
        />
      </div>
      <div>
        <label className='block text-sm font-medium text-stone-700 mb-1'>
          Birthdate
        </label>
        <input
          type='date'
          required
          value={form.birthdate}
          onChange={(e) => setForm({ ...form, birthdate: e.target.value })}
          className='input w-full'
        />
      </div>
      <div>
        <label className='block text-sm font-medium text-stone-700 mb-1'>
          Dietary Goals
        </label>
        <input
          type='text'
          value={form.dietary_goals}
          onChange={(e) => setForm({ ...form, dietary_goals: e.target.value })}
          placeholder='e.g., More protein, less sugar'
          className='input w-full'
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
        <label className='block text-sm font-medium text-stone-700 mb-1'>
          Notes
        </label>
        <textarea
          value={form.notes}
          onChange={(e) => setForm({ ...form, notes: e.target.value })}
          className='input w-full'
          rows={2}
        />
      </div>
      <div className='flex gap-2'>
        <button
          type='submit'
          disabled={!isValid}
          className='btn-md btn-primary'
        >
          {submitLabel}
        </button>
        <button
          type='button'
          onClick={onCancel}
          className='btn-md btn-outline'
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
  const { toast } = useToast()

  const [isAdding, setIsAdding] = useState(false)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<string | null>(null)

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (confirmingDeleteId) {
          setConfirmingDeleteId(null)
        } else if (editingId) {
          setEditingId(null)
        } else if (isAdding) {
          setIsAdding(false)
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [editingId, isAdding, confirmingDeleteId])

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
      onSuccess: () => {
        setIsAdding(false)
        toast('Family member added')
      },
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
      onSuccess: () => {
        setEditingId(null)
        toast('Changes saved')
      },
    })
  }

  const handleDelete = (id: string) => {
    deleteMutation.mutate(id, {
      onSuccess: () => {
        setConfirmingDeleteId(null)
        toast('Family member removed')
      },
    })
  }

  if (isLoading) {
    return <div className='p-6 text-stone-500 animate-pulse'>Loading family members...</div>
  }

  if (error) {
    return (
      <div className='p-6'>
        <div className='panel-error text-red-700 text-sm'>
          Failed to load family members: {String(error)}
        </div>
      </div>
    )
  }

  return (
    <div className='p-6'>
      <div className='flex items-center justify-between mb-6'>
        <h1 className='text-2xl font-bold text-stone-900'>Family Members</h1>
        {!isAdding && (
          <button
            onClick={() => setIsAdding(true)}
            className='btn-md btn-primary'
          >
            <IconPlus className='w-4 h-4' />
            Add Person
          </button>
        )}
      </div>

      {isAdding && (
        <div className='card p-4 mb-6 animate-slide-up'>
          <h3 className='font-semibold text-lg mb-3'>Add Family Member</h3>
          <PersonForm
            initialData={emptyForm}
            onSubmit={handleCreate}
            onCancel={() => setIsAdding(false)}
            submitLabel='Add Person'
          />
          {createMutation.error && (
            <div className='mt-2 panel-error text-red-700 text-sm'>
              {String(createMutation.error)}
            </div>
          )}
        </div>
      )}

      {people?.length === 0 && !isAdding && (
        <EmptyState
          emoji='👨‍👩‍👧‍👦'
          title='Your family awaits'
          description='Add your first family member to start planning meals together.'
          action={{ label: 'Add Person', onClick: () => setIsAdding(true) }}
        />
      )}

      <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
        {people?.map((person) => {
          const parsed = parsePerson(person)

          if (editingId === person.id) {
            return (
              <div
                key={person.id}
                className='panel-primary animate-slide-up'
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
                  <div className='mt-2 panel-error text-red-700 text-sm'>
                    {String(updateMutation.error)}
                  </div>
                )}
              </div>
            )
          }

          return (
            <div
              key={person.id}
              className='card p-4 animate-slide-up'
            >
              <div className='flex items-start justify-between'>
                <h3 className='font-semibold text-lg'>{person.name}</h3>
                <div className='flex gap-2'>
                  <button
                    onClick={() => setEditingId(person.id)}
                    className='text-primary-600 text-sm hover:underline'
                    aria-label={`Edit ${person.name}`}
                  >
                    <IconEdit className='w-4 h-4' />
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
                          className='text-stone-500 hover:underline'
                        >
                          No
                        </button>
                      </span>
                    )
                    : (
                      <button
                        onClick={() => setConfirmingDeleteId(person.id)}
                        className='btn-sm btn-danger'
                        aria-label={`Delete ${person.name}`}
                      >
                        <IconTrash className='w-4 h-4' />
                      </button>
                    )}
                </div>
              </div>
              <p className='text-sm text-stone-500 mt-1'>
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
                  <span className='text-sm text-stone-600'>
                    {parsed.favorites.join(', ')}
                  </span>
                </div>
              )}
              {parsed.dislikes.length > 0 && (
                <div className='mt-1'>
                  <span className='text-sm font-medium'>Dislikes:</span>
                  <span className='text-sm text-stone-600'>
                    {parsed.dislikes.join(', ')}
                  </span>
                </div>
              )}
              {person.notes && (
                <p className='text-sm text-stone-500 mt-2 italic'>
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
