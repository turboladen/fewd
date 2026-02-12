import { useQuery } from '@tanstack/react-query'
import { api } from '../lib/api'
import type { AggregatedIngredient } from '../types/shopping'

export function useShoppingList(startDate: string, endDate: string) {
  return useQuery({
    queryKey: ['shopping', startDate, endDate],
    queryFn: () =>
      api.get<AggregatedIngredient[]>('/shopping-list?start_date=' + startDate + '&end_date=' + endDate),
    enabled: !!startDate && !!endDate,
  })
}
