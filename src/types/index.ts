export interface NoteMeta {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  template_id?: string;
  title_edited?: boolean;
}

export interface TemplateMeta {
  id: string;
  name: string;
  preset: boolean;        // true = built-in, false = user-created
  created_at: string;
}

export type ViewMode = "edit" | "preview" | "split";
