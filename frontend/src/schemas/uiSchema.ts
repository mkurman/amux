import { z } from "zod";

export interface UINodeBuilderMeta {
  editable?: boolean;
  locked?: boolean;
  droppable?: boolean;
  resizable?: boolean;
  resizeAxis?: "both" | "horizontal" | "vertical";
  movable?: boolean;
  align?: "start" | "center" | "end" | "stretch";
  slot?: string;
  data?: Record<string, unknown>;
}

export interface UIBlockBuilderMeta {
  category?: string;
  icon?: string;
  editable?: boolean;
  data?: Record<string, unknown>;
}

export interface UIViewBuilderMeta {
  canvas?: Record<string, unknown>;
  data?: Record<string, unknown>;
}

export interface UIViewNode {
  id?: string;
  type?: string;
  use?: string;
  props?: Record<string, unknown>;
  command?: string;
  children?: UIViewNode[];
  builder?: UINodeBuilderMeta;
}

export interface UIViewBlockDefinition {
  title?: string;
  description?: string;
  layout: UIViewNode;
  defaults?: Record<string, unknown>;
  builder?: UIBlockBuilderMeta;
}

export interface ViewDocument {
  schemaVersion?: number;
  title?: string;
  when?: string;
  blocks?: Record<string, UIViewBlockDefinition>;
  layout: UIViewNode;
  fallback?: UIViewNode;
  builder?: UIViewBuilderMeta;
}

export interface UIComponentNode {
  nodeId?: string;
  type: string;
  props?: Record<string, unknown>;
  command?: string;
  children?: UIComponentNode[];
  builder?: UINodeBuilderMeta;
}

export interface ViewConfig {
  schemaVersion?: number;
  title?: string;
  when?: string;
  layout: UIComponentNode;
  fallback?: UIComponentNode;
}

export const UIPropSchema = z.record(z.string(), z.any());

const UINodeBuilderMetaSchema: z.ZodType<UINodeBuilderMeta> = z.object({
  editable: z.boolean().optional(),
  locked: z.boolean().optional(),
  droppable: z.boolean().optional(),
  resizable: z.boolean().optional(),
  resizeAxis: z.enum(["both", "horizontal", "vertical"]).optional(),
  movable: z.boolean().optional(),
  align: z.enum(["start", "center", "end", "stretch"]).optional(),
  slot: z.string().optional(),
  data: UIPropSchema.optional(),
});

const UIBlockBuilderMetaSchema: z.ZodType<UIBlockBuilderMeta> = z.object({
  category: z.string().optional(),
  icon: z.string().optional(),
  editable: z.boolean().optional(),
  data: UIPropSchema.optional(),
});

const UIViewBuilderMetaSchema: z.ZodType<UIViewBuilderMeta> = z.object({
  canvas: UIPropSchema.optional(),
  data: UIPropSchema.optional(),
});

export const ViewNodeSchema: z.ZodType<UIViewNode> = z.lazy(() =>
  z.object({
    id: z.string().min(1).optional(),
    type: z.string().min(1).optional(),
    use: z.string().min(1).optional(),
    props: UIPropSchema.optional(),
    command: z.string().optional(),
    children: z.array(ViewNodeSchema).optional(),
    builder: UINodeBuilderMetaSchema.optional(),
  }).superRefine((node, ctx) => {
    if (!node.type && !node.use) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: "A view node must define either 'type' or 'use'.",
      });
    }

    if (node.type && node.use) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: "A view node cannot define both 'type' and 'use'.",
      });
    }
  }),
);

const ViewBlockDefinitionSchema: z.ZodType<UIViewBlockDefinition> = z.object({
  title: z.string().optional(),
  description: z.string().optional(),
  layout: ViewNodeSchema,
  defaults: UIPropSchema.optional(),
  builder: UIBlockBuilderMetaSchema.optional(),
});

export const ComponentNodeSchema: z.ZodType<UIComponentNode> = z.lazy(() =>
  z.object({
    nodeId: z.string().min(1).optional(),
    type: z.string(),
    props: UIPropSchema.optional(),
    command: z.string().optional(),
    children: z.array(ComponentNodeSchema).optional(),
    builder: UINodeBuilderMetaSchema.optional(),
  }),
);

export const ViewDocumentSchema: z.ZodType<ViewDocument> = z.object({
  schemaVersion: z.number().int().positive().optional(),
  title: z.string().optional(),
  when: z.string().optional(),
  blocks: z.record(z.string(), ViewBlockDefinitionSchema).optional(),
  layout: ViewNodeSchema,
  fallback: ViewNodeSchema.optional(),
  builder: UIViewBuilderMetaSchema.optional(),
});

export const ViewConfigSchema: z.ZodType<ViewConfig> = z.object({
  schemaVersion: z.number().int().positive().optional(),
  title: z.string().optional(),
  when: z.string().optional(),
  layout: ComponentNodeSchema,
  fallback: ComponentNodeSchema.optional(),
});
