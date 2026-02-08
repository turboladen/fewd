import { useQuery } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/core'
import type { AggregatedIngredient } from '../types/shopping'

export function useShoppingList(startDate: string, endDate: string) {
  return useQuery({
    queryKey: ['shopping', startDate, endDate],
    queryFn: () => invoke<AggregatedIngredient[]>('get_shopping_list', { startDate, endDate }),
    enabled: !!startDate && !!endDate,
  })
}
