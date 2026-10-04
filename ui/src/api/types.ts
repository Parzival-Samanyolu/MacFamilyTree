export type Sex = 'M' | 'F' | 'X' | 'U' | ''

export interface Summary {
  id: string
  given: string
  surname: string
  name: string
  sex: Sex
  bookmarked: boolean
  color: string | null
  private: boolean
  birth_text: string | null
  birth_year: number | null
  death_text: string | null
  death_year: number | null
  living: boolean
  life: string
}

export interface Status {
  open: boolean
  path?: string | null
  persons?: number
  families?: number
  can_undo?: boolean
  can_redo?: boolean
  undo_label?: string | null
}

export interface Citation {
  id: string
  source_id: string
  source_title: string
  page: string | null
  quality: string | null
}
export interface NoteItem {
  id: string
  link_id: string
  title: string | null
  body: string
}

export interface EventItem {
  id: string
  owner_type: string
  owner_id: string
  kind: string
  custom_kind: string | null
  value: string | null
  date_text: string | null
  date_gedcom: string | null
  date_sort: number | null
  place_id: string | null
  place_text: string | null
  cause: string | null
  agency: string | null
  age: number | null
  citations: Citation[]
  notes: NoteItem[]
}

export interface NameItem {
  id: string
  person_id: string
  kind: string
  prefix: string
  given: string
  nickname: string
  surname_prefix: string
  surname: string
  suffix: string
  sort_order: number
}

export type ChildSummary = Summary & { link_id: string; rel_type: string }

export interface PartnerFamily {
  id: string
  rel_type: string
  partners: Summary[]
  children: ChildSummary[]
  events: EventItem[]
  notes: NoteItem[]
  citations: Citation[]
}
export interface ChildFamily {
  id: string
  link_id: string
  rel_type: string
  parents: Summary[]
  siblings: Summary[]
}

export interface PersonDetail {
  person: {
    id: string
    sex: Sex
    is_private: boolean
    bookmarked: boolean
    color: string | null
    ref_no: string | null
    primary_media?: string | null
    living_override: number | null
    living: boolean
    created: number | null
    modified: number | null
  }
  summary: Summary
  names: NameItem[]
  events: EventItem[]
  partner_families: PartnerFamily[]
  child_families: ChildFamily[]
  notes: NoteItem[]
  citations: Citation[]
  associations: { id: string; role: string; notes: string | null; other: Summary }[]
  tasks: { id: string; title: string; status: string }[]
  media: { id: string; path: string; caption: string | null }[]
}

export interface LNode {
  person_id: string
  x: number
  y: number
  w: number
  h: number
  generation: number
  dup_of: number | null
  has_more: boolean
  is_spouse: boolean
}
export interface LUnion {
  family_id: string
  x: number
  y: number
}
export interface LEdge {
  kind: 'Partner' | 'Child'
  from: number
  from_union: boolean
  to: number
  to_union: boolean
  link: string
}
export interface Layout {
  nodes: LNode[]
  unions: LUnion[]
  edges: LEdge[]
  width: number
  height: number
}
export interface TreeResult {
  layout: Layout
  people: Record<string, Summary>
}

export interface Finding {
  id: string
  rule: string
  severity: 'Info' | 'Warning' | 'Error'
  entity_type: string
  entity_id: string
  related: string[]
  message: string
  fix: Record<string, unknown> | null
  person_id: string | null
}

export interface DuplicateCandidate {
  a: Summary
  b: Summary
  score: number
  reasons: string[]
}

export interface Bucket {
  label: string
  count: number
  ids: string[]
}
export interface Stats {
  counts: Record<string, number>
  sex: Bucket[]
  age_at_death: Bucket[]
  lifespan_by_century: [number, number, number][]
  avg_marriage_age: number | null
  children_per_family: Bucket[]
  top_given_names: [string, number][]
  top_surnames: [string, number][]
  top_occupations: [string, number][]
  top_places: [string, number][]
  birth_months: Bucket[]
  generation_depth: number
  avg_completeness: number
  source_coverage_pct: number
  longest_lived: [string, number][]
  largest_families: [string, number][]
}

export interface ImportReport {
  version: string
  charset: string
  persons: number
  families: number
  events: number
  places: number
  sources: number
  repositories: number
  notes: number
  media: number
  preserved_structures: number
  issues: { severity: 'info' | 'warning' | 'error'; line: number; message: string }[]
}
