---
name: feedback-widget
description: "Set up an in-app feedback widget with screenshot capture, canvas annotation, and GitHub issue creation. Use when adding feedback/bug-reporting UI to any Next.js project."
user_invocable: true
---

# Feedback Widget

Set up a floating feedback button that lets users capture a screenshot, annotate it (freehand draw, arrows, rectangles, crop), and submit it as a GitHub issue — all without leaving the app.

## Prerequisites

- **Next.js App Router** project (13+)
- **shadcn/ui** installed (Button, Sheet components)
- **lucide-react** installed

## Setup Steps

### 1. Understand the target project

Before writing any code, determine:

- **Auth system**: How does the project authenticate users? (Supabase, NextAuth, Clerk, custom JWT, none)
- **UI components path**: Where are shadcn components? (usually `src/components/ui/` or `components/ui/`)
- **App metadata**: What project-specific context should be captured with feedback? (current page URL is always captured; additional context like selected items, user settings, etc. is project-specific)

### 2. Install dependencies

```bash
npm install html2canvas-pro
# or: pnpm add html2canvas-pro / bun add html2canvas-pro
```

Ensure these shadcn components are installed:
```bash
npx shadcn@latest add button sheet
```

### 3. Create the AnnotationEditor component

Create `annotation-editor.tsx` in the project's component directory.

This component is **fully portable** — copy it verbatim from `## AnnotationEditor Reference` below. It has no project-specific dependencies beyond shadcn `Button` and lucide icons.

Key features:
- Dual-canvas architecture (base image + draw overlay) for efficient rendering
- Tools: freehand draw, arrow, rectangle, crop
- Undo/clear via ImageData history stack
- Retina display support via `devicePixelRatio`
- Outputs a single composited PNG data URL on "Done"

### 4. Create the FeedbackButton component

Create `feedback-button.tsx` in the project's component directory. This needs **project-specific adaptation**:

**Auth gating**: Replace the auth check with the project's auth system:

```tsx
// Supabase example
const supabase = createClient();
const { data: { user } } = await supabase.auth.getUser();
setIsAuthenticated(!!user);

// NextAuth example
const { data: session } = useSession();
setIsAuthenticated(!!session);

// Clerk example
const { isSignedIn } = useUser();
setIsAuthenticated(isSignedIn ?? false);

// No auth (always show)
setIsAuthenticated(true);
```

**Metadata collection**: Adapt the metadata object in `handleSubmit` to capture project-relevant context. Always include:
- `url` — current page path + query params
- `viewport` — window dimensions
- `userAgent` — browser string

Add any project-specific context (selected items, active filters, user role, etc.).

**Reference implementation:** See `## FeedbackButton Reference` below.

### 5. Create the API route

Create the API route at `app/api/feedback/route.ts`.

This route:
1. Validates the user is authenticated (adapt auth check to project's system)
2. Uploads the screenshot PNG to the GitHub repo's `.feedback/` directory via GitHub Contents API
3. Creates a GitHub issue with description, embedded screenshot, and metadata

**Auth check adaptation**: Same as step 4 — use the project's server-side auth.

**Reference implementation:** See `## API Route Reference` below.

### 6. Set environment variables

Add to `.env` (or `.env.local`):

```
GITHUB_TOKEN='ghp_...'
GITHUB_FEEDBACK_REPO='owner/repo'
```

The GitHub token needs `repo` scope (to create issues and upload files).

**Important**: These are server-only variables (no `NEXT_PUBLIC_` prefix). Never expose the GitHub token to the client.

### 7. Wire up in layout

Add `FeedbackButton` to the root layout, wrapped in `<Suspense>` (it uses `useSearchParams`):

```tsx
import { Suspense } from "react";
import { FeedbackButton } from "@/components/feedback-button";

// Inside the layout body, before closing </body>:
<Suspense>
  <FeedbackButton />
</Suspense>
```

### 8. Add the feedback label to GitHub

Create a "feedback" label in the target GitHub repo so issues get categorized:

```bash
gh label create feedback --color 0075ca --description "In-app user feedback" --repo owner/repo
```

### 9. Verify

1. Run the dev server
2. Confirm the floating button appears (bottom-right) for authenticated users
3. Click it → capture screenshot → annotate → describe → submit
4. Verify a GitHub issue is created with the screenshot embedded

---

## AnnotationEditor Reference

```tsx
"use client";

import { useRef, useState, useEffect, useCallback } from "react";
import { Button } from "@/components/ui/button";
import {
  Pencil,
  MoveUpRight,
  Square,
  Crop,
  Undo2,
  Trash2,
  Check,
  X,
} from "lucide-react";

type Tool = "freehand" | "arrow" | "rectangle" | "crop";

type Props = {
  imageData: string; // base64 PNG data URL
  onDone: (annotatedImage: string) => void;
  onCancel: () => void;
};

const STROKE_COLOR = "#ef4444"; // red-500
const STROKE_WIDTH = 3;

const tools: { id: Tool; icon: typeof Pencil; label: string }[] = [
  { id: "freehand", icon: Pencil, label: "Draw" },
  { id: "arrow", icon: MoveUpRight, label: "Arrow" },
  { id: "rectangle", icon: Square, label: "Rectangle" },
  { id: "crop", icon: Crop, label: "Crop" },
];

export function AnnotationEditor({ imageData, onDone, onCancel }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const baseCanvasRef = useRef<HTMLCanvasElement>(null);
  const drawCanvasRef = useRef<HTMLCanvasElement>(null);
  const [tool, setTool] = useState<Tool>("freehand");
  const [isDrawing, setIsDrawing] = useState(false);
  const startPosRef = useRef({ x: 0, y: 0 });
  const [history, setHistory] = useState<ImageData[]>([]);
  const [canvasSize, setCanvasSize] = useState({ width: 0, height: 0 });
  const cropRegionRef = useRef<{
    x: number;
    y: number;
    w: number;
    h: number;
  } | null>(null);
  const preShapeSnapshot = useRef<ImageData | null>(null);

  useEffect(() => {
    const img = new Image();
    img.onload = () => {
      const container = containerRef.current;
      if (!container) return;

      const dpr = window.devicePixelRatio || 1;
      const maxW = container.clientWidth - 48;
      const maxH = container.clientHeight - 120;
      const scale = Math.min(1, maxW / img.width, maxH / img.height);
      const w = Math.round(img.width * scale);
      const h = Math.round(img.height * scale);
      setCanvasSize({ width: w, height: h });

      const baseCtx = baseCanvasRef.current?.getContext("2d");
      const drawCtx = drawCanvasRef.current?.getContext("2d");
      if (!baseCtx || !drawCtx) return;

      baseCanvasRef.current!.width = w * dpr;
      baseCanvasRef.current!.height = h * dpr;
      drawCanvasRef.current!.width = w * dpr;
      drawCanvasRef.current!.height = h * dpr;

      baseCtx.scale(dpr, dpr);
      drawCtx.scale(dpr, dpr);
      baseCtx.drawImage(img, 0, 0, w, h);
    };
    img.src = imageData;
  }, [imageData]);

  function getPos(e: React.MouseEvent<HTMLCanvasElement>) {
    const canvas = drawCanvasRef.current;
    if (!canvas) return { x: 0, y: 0 };
    const rect = canvas.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  }

  const handleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const pos = getPos(e);
      setIsDrawing(true);
      startPosRef.current = pos;

      const ctx = drawCanvasRef.current?.getContext("2d");
      if (!ctx) return;

      preShapeSnapshot.current = ctx.getImageData(
        0, 0, drawCanvasRef.current!.width, drawCanvasRef.current!.height
      );

      if (tool === "freehand") {
        ctx.beginPath();
        ctx.moveTo(pos.x, pos.y);
        ctx.strokeStyle = STROKE_COLOR;
        ctx.lineWidth = STROKE_WIDTH;
        ctx.lineCap = "round";
        ctx.lineJoin = "round";
      }
    },
    [tool, canvasSize],
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      if (!isDrawing) return;
      const pos = getPos(e);
      const ctx = drawCanvasRef.current?.getContext("2d");
      if (!ctx) return;

      if (tool === "freehand") {
        ctx.lineTo(pos.x, pos.y);
        ctx.stroke();
      } else if (tool === "arrow" || tool === "rectangle") {
        if (preShapeSnapshot.current) {
          ctx.putImageData(preShapeSnapshot.current, 0, 0);
        }
        ctx.strokeStyle = STROKE_COLOR;
        ctx.lineWidth = STROKE_WIDTH;
        ctx.lineCap = "round";

        if (tool === "rectangle") {
          ctx.strokeRect(
            startPosRef.current.x, startPosRef.current.y,
            pos.x - startPosRef.current.x, pos.y - startPosRef.current.y
          );
        } else {
          drawArrow(ctx, startPosRef.current.x, startPosRef.current.y, pos.x, pos.y);
        }
      } else if (tool === "crop") {
        if (preShapeSnapshot.current) {
          ctx.putImageData(preShapeSnapshot.current, 0, 0);
        }
        const x = Math.min(startPosRef.current.x, pos.x);
        const y = Math.min(startPosRef.current.y, pos.y);
        const w = Math.abs(pos.x - startPosRef.current.x);
        const h = Math.abs(pos.y - startPosRef.current.y);
        ctx.fillStyle = "rgba(0,0,0,0.4)";
        ctx.fillRect(0, 0, canvasSize.width, canvasSize.height);
        ctx.clearRect(x, y, w, h);
        ctx.setLineDash([6, 3]);
        ctx.strokeStyle = "#fff";
        ctx.lineWidth = 2;
        ctx.strokeRect(x, y, w, h);
        ctx.setLineDash([]);
        cropRegionRef.current = { x, y, w, h };
      }
    },
    [isDrawing, tool, canvasSize],
  );

  function applyCrop(region: { x: number; y: number; w: number; h: number }) {
    const baseCtx = baseCanvasRef.current?.getContext("2d");
    const drawCtx = drawCanvasRef.current?.getContext("2d");
    if (!baseCtx || !drawCtx) return;

    const dpr = window.devicePixelRatio || 1;
    const baseData = baseCtx.getImageData(
      region.x * dpr, region.y * dpr, region.w * dpr, region.h * dpr
    );
    if (preShapeSnapshot.current) {
      drawCtx.putImageData(preShapeSnapshot.current, 0, 0);
    }
    const drawData = drawCtx.getImageData(
      region.x * dpr, region.y * dpr, region.w * dpr, region.h * dpr
    );

    baseCanvasRef.current!.width = region.w * dpr;
    baseCanvasRef.current!.height = region.h * dpr;
    drawCanvasRef.current!.width = region.w * dpr;
    drawCanvasRef.current!.height = region.h * dpr;
    setCanvasSize({ width: region.w, height: region.h });

    baseCtx.putImageData(baseData, 0, 0);
    drawCtx.putImageData(drawData, 0, 0);
    baseCtx.scale(dpr, dpr);
    drawCtx.scale(dpr, dpr);
    cropRegionRef.current = null;
    setHistory([]);
  }

  const handleMouseUp = useCallback(() => {
    if (!isDrawing) return;
    setIsDrawing(false);
    const ctx = drawCanvasRef.current?.getContext("2d");
    if (!ctx) return;

    const crop = cropRegionRef.current;
    if (tool === "crop" && crop && crop.w > 10 && crop.h > 10) {
      applyCrop(crop);
    } else if (tool !== "crop") {
      const snapshot = ctx.getImageData(
        0, 0, drawCanvasRef.current!.width, drawCanvasRef.current!.height
      );
      setHistory((prev) => [...prev, snapshot]);
    }

    preShapeSnapshot.current = null;
  }, [isDrawing, tool, canvasSize]);

  function performUndo() {
    const ctx = drawCanvasRef.current?.getContext("2d");
    if (!ctx || history.length === 0) return;
    const prev = [...history];
    prev.pop();
    setHistory(prev);
    ctx.clearRect(0, 0, canvasSize.width, canvasSize.height);
    if (prev.length > 0) {
      ctx.putImageData(prev[prev.length - 1], 0, 0);
    }
  }

  function performClear() {
    const ctx = drawCanvasRef.current?.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvasSize.width, canvasSize.height);
    setHistory([]);
  }

  function handleDone() {
    const dpr = window.devicePixelRatio || 1;
    const canvas = document.createElement("canvas");
    canvas.width = canvasSize.width * dpr;
    canvas.height = canvasSize.height * dpr;
    const ctx = canvas.getContext("2d")!;
    ctx.drawImage(baseCanvasRef.current!, 0, 0);
    ctx.drawImage(drawCanvasRef.current!, 0, 0);
    onDone(canvas.toDataURL("image/png"));
  }

  return (
    <div
      ref={containerRef}
      className="fixed inset-0 z-[100] flex flex-col items-center justify-center bg-black/80"
    >
      <div className="mb-3 flex items-center gap-1 rounded-lg bg-background p-1 shadow-lg">
        {tools.map(({ id, icon: Icon, label }) => (
          <Button
            key={id}
            variant={tool === id ? "default" : "ghost"}
            size="sm"
            onClick={() => setTool(id)}
            title={label}
          >
            <Icon className="size-4" />
            <span className="hidden sm:inline">{label}</span>
          </Button>
        ))}
        <div className="bg-border mx-1 h-6 w-px" />
        <Button
          variant="ghost"
          size="sm"
          onClick={performUndo}
          disabled={history.length === 0}
          title="Undo"
        >
          <Undo2 className="size-4" />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={performClear}
          disabled={history.length === 0}
          title="Clear all"
        >
          <Trash2 className="size-4" />
        </Button>
        <div className="bg-border mx-1 h-6 w-px" />
        <Button variant="ghost" size="sm" onClick={onCancel}>
          <X className="size-4" />
          Cancel
        </Button>
        <Button size="sm" onClick={handleDone}>
          <Check className="size-4" />
          Done
        </Button>
      </div>

      <div className="relative" style={{ width: canvasSize.width, height: canvasSize.height }}>
        <canvas
          ref={baseCanvasRef}
          className="absolute top-0 left-0 rounded"
          style={{ width: canvasSize.width, height: canvasSize.height }}
        />
        <canvas
          ref={drawCanvasRef}
          className="absolute top-0 left-0 cursor-crosshair rounded"
          style={{ width: canvasSize.width, height: canvasSize.height }}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseUp}
        />
      </div>
    </div>
  );
}

function drawArrow(
  ctx: CanvasRenderingContext2D,
  fromX: number, fromY: number,
  toX: number, toY: number,
) {
  const headLen = 14;
  const angle = Math.atan2(toY - fromY, toX - fromX);

  ctx.beginPath();
  ctx.moveTo(fromX, fromY);
  ctx.lineTo(toX, toY);
  ctx.stroke();

  ctx.beginPath();
  ctx.moveTo(toX, toY);
  ctx.lineTo(
    toX - headLen * Math.cos(angle - Math.PI / 6),
    toY - headLen * Math.sin(angle - Math.PI / 6),
  );
  ctx.lineTo(
    toX - headLen * Math.cos(angle + Math.PI / 6),
    toY - headLen * Math.sin(angle + Math.PI / 6),
  );
  ctx.closePath();
  ctx.fillStyle = ctx.strokeStyle;
  ctx.fill();
}
```

## FeedbackButton Reference

```tsx
"use client";

import { useState, useEffect, useCallback } from "react";
import { usePathname, useSearchParams } from "next/navigation";
import { MessageSquarePlus, Camera, Loader2, Send } from "lucide-react";
import html2canvas from "html2canvas-pro";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
  SheetFooter,
} from "@/components/ui/sheet";
import { AnnotationEditor } from "@/components/annotation-editor";

// ADAPT: Import your auth client here
// import { createClient } from "@/lib/supabase/client";
// import { useSession } from "next-auth/react";
// import { useUser } from "@clerk/nextjs";

export function FeedbackButton() {
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [open, setOpen] = useState(false);
  const [screenshot, setScreenshot] = useState<string | null>(null);
  const [annotatedImage, setAnnotatedImage] = useState<string | null>(null);
  const [showAnnotation, setShowAnnotation] = useState(false);
  const [description, setDescription] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // ADAPT: Replace with your project's auth check
    // Supabase:
    //   const supabase = createClient();
    //   supabase.auth.getUser().then(({ data: { user } }) => setIsAuthenticated(!!user));
    // NextAuth:
    //   setIsAuthenticated(!!session);
    // No auth:
    setIsAuthenticated(true);
  }, []);

  const captureScreenshot = useCallback(async () => {
    setOpen(false);
    await new Promise((r) => setTimeout(r, 350)); // wait for sheet close animation

    try {
      const canvas = await html2canvas(document.body, {
        useCORS: true,
        logging: false,
        scale: window.devicePixelRatio || 1,
        windowWidth: document.documentElement.scrollWidth,
        windowHeight: document.documentElement.scrollHeight,
      });
      const dataUrl = canvas.toDataURL("image/png");
      setScreenshot(dataUrl);
      setShowAnnotation(true);
    } catch (err) {
      console.error("Screenshot capture failed:", err);
      setOpen(true);
      setError("Failed to capture screenshot. Please try again.");
    }
  }, []);

  function handleAnnotationDone(annotated: string) {
    setAnnotatedImage(annotated);
    setShowAnnotation(false);
    setOpen(true);
  }

  function handleAnnotationCancel() {
    setShowAnnotation(false);
    setScreenshot(null);
    setOpen(true);
  }

  async function handleSubmit() {
    if (!description.trim()) return;
    setSubmitting(true);
    setError(null);

    try {
      // ADAPT: Add any project-specific metadata here
      const metadata = {
        url: `${pathname}${searchParams.toString() ? `?${searchParams.toString()}` : ""}`,
        viewport: `${window.innerWidth}x${window.innerHeight}`,
        userAgent: navigator.userAgent,
      };

      const res = await fetch("/api/feedback", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          image: annotatedImage || screenshot,
          description,
          metadata,
        }),
      });

      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.error || `Request failed (${res.status})`);
      }

      setSubmitted(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to submit feedback");
    } finally {
      setSubmitting(false);
    }
  }

  function resetState() {
    setScreenshot(null);
    setAnnotatedImage(null);
    setDescription("");
    setSubmitted(false);
    setError(null);
    setOpen(false);
  }

  if (!isAuthenticated) return null;

  return (
    <>
      <Button
        onClick={() => setOpen(true)}
        className="fixed bottom-6 right-6 z-50 size-12 rounded-full shadow-lg"
        size="icon"
        title="Send feedback"
      >
        <MessageSquarePlus className="size-5" />
      </Button>

      <Sheet open={open} onOpenChange={(v) => (v ? setOpen(true) : resetState())}>
        <SheetContent side="right" className="flex flex-col sm:max-w-md">
          <SheetHeader>
            <SheetTitle>Send Feedback</SheetTitle>
            <SheetDescription>
              Capture a screenshot, annotate it, and describe what looks off.
            </SheetDescription>
          </SheetHeader>

          {submitted ? (
            <div className="flex flex-1 flex-col items-center justify-center gap-4 p-4">
              <p className="text-center text-sm text-muted-foreground">
                Feedback submitted! A GitHub issue has been created. Thank you.
              </p>
              <Button variant="outline" onClick={resetState}>
                Close
              </Button>
            </div>
          ) : (
            <>
              <div className="flex flex-1 flex-col gap-4 overflow-y-auto p-4">
                {annotatedImage ? (
                  <div className="space-y-2">
                    <p className="text-sm font-medium">Screenshot</p>
                    <img
                      src={annotatedImage}
                      alt="Annotated screenshot"
                      className="w-full rounded border"
                    />
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => {
                        setAnnotatedImage(null);
                        setShowAnnotation(true);
                        setOpen(false);
                      }}
                    >
                      Re-annotate
                    </Button>
                  </div>
                ) : (
                  <Button variant="outline" onClick={captureScreenshot}>
                    <Camera className="size-4" />
                    Capture Screenshot
                  </Button>
                )}

                <div className="space-y-2">
                  <label
                    htmlFor="feedback-description"
                    className="text-sm font-medium"
                  >
                    What looks off?
                  </label>
                  <textarea
                    id="feedback-description"
                    className="border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:ring-ring flex min-h-[100px] w-full rounded-md border px-3 py-2 text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50"
                    placeholder="Describe the issue..."
                    value={description}
                    onChange={(e) => setDescription(e.target.value)}
                  />
                </div>

                {error && (
                  <p className="text-sm text-destructive">{error}</p>
                )}
              </div>

              <SheetFooter>
                <Button
                  onClick={handleSubmit}
                  disabled={!description.trim() || submitting}
                  className="w-full"
                >
                  {submitting ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <Send className="size-4" />
                  )}
                  {submitting ? "Submitting..." : "Submit Feedback"}
                </Button>
              </SheetFooter>
            </>
          )}
        </SheetContent>
      </Sheet>

      {showAnnotation && screenshot && (
        <AnnotationEditor
          imageData={screenshot}
          onDone={handleAnnotationDone}
          onCancel={handleAnnotationCancel}
        />
      )}
    </>
  );
}
```

## API Route Reference

```tsx
// src/app/api/feedback/route.ts

// ADAPT: Import your server-side auth here
// import { createClient } from "@/lib/supabase/server";
// import { getServerSession } from "next-auth";
// import { auth } from "@clerk/nextjs/server";

type FeedbackMetadata = {
  url: string;
  viewport: string;
  userAgent: string;
  [key: string]: unknown; // project-specific fields
};

type FeedbackBody = {
  image: string; // base64 data URL
  description: string;
  metadata: FeedbackMetadata;
};

export async function POST(req: Request) {
  // ADAPT: Replace with your project's server-side auth check
  // Supabase:
  //   const supabase = await createClient();
  //   const { data: { user } } = await supabase.auth.getUser();
  //   if (!user) return new Response("Unauthorized", { status: 401 });
  //   const userEmail = user.email;
  // NextAuth:
  //   const session = await getServerSession();
  //   if (!session) return new Response("Unauthorized", { status: 401 });
  //   const userEmail = session.user?.email;
  // No auth:
  const userEmail = "anonymous";

  const body: FeedbackBody = await req.json();
  if (!body.description?.trim()) {
    return Response.json({ error: "Description is required" }, { status: 400 });
  }

  const token = process.env.GITHUB_TOKEN;
  const repo = process.env.GITHUB_FEEDBACK_REPO;
  if (!token || !repo) {
    return Response.json(
      { error: "GitHub integration not configured" },
      { status: 500 },
    );
  }

  const headers = {
    Authorization: `Bearer ${token}`,
    Accept: "application/vnd.github.v3+json",
    "Content-Type": "application/json",
  };

  // Upload screenshot to repo
  let screenshotUrl = "";
  if (body.image) {
    const base64Data = body.image.replace(/^data:image\/\w+;base64,/, "");
    const filename = `feedback-${Date.now()}-${Math.random().toString(36).slice(2, 7)}.png`;
    const path = `.feedback/${filename}`;

    const uploadRes = await fetch(
      `https://api.github.com/repos/${repo}/contents/${path}`,
      {
        method: "PUT",
        headers,
        body: JSON.stringify({
          message: `feedback: upload screenshot ${filename}`,
          content: base64Data,
        }),
      },
    );

    if (uploadRes.ok) {
      const uploadData = await uploadRes.json();
      screenshotUrl = uploadData.content.download_url;
    } else {
      console.error("Screenshot upload failed:", uploadRes.status, await uploadRes.text());
    }
  }

  // Build issue body — ADAPT: format project-specific metadata fields here
  const meta = body.metadata;
  const metadataLines = [
    `- Page: ${meta.url}`,
    `- User: ${userEmail}`,
    `- Viewport: ${meta.viewport}`,
    `- Browser: ${meta.userAgent}`,
  ];

  const issueBody = [
    `**Description:** ${body.description}`,
    "",
    screenshotUrl ? `**Screenshot:**\n![screenshot](${screenshotUrl})` : "",
    "",
    "**Metadata:**",
    ...metadataLines,
  ]
    .filter(Boolean)
    .join("\n");

  // Create GitHub issue
  const issueRes = await fetch(`https://api.github.com/repos/${repo}/issues`, {
    method: "POST",
    headers,
    body: JSON.stringify({
      title: `[Feedback] ${body.description.slice(0, 80)}`,
      body: issueBody,
      labels: ["feedback"],
    }),
  });

  if (!issueRes.ok) {
    const err = await issueRes.text();
    console.error("GitHub issue creation failed:", err);
    return Response.json(
      { error: "Failed to create GitHub issue" },
      { status: 502 },
    );
  }

  const issue = await issueRes.json();
  return Response.json({
    issueUrl: issue.html_url,
    issueNumber: issue.number,
    screenshotAttached: !!screenshotUrl,
  });
}
```

## Test Reference

```tsx
// __tests__/api/feedback.test.ts
import { describe, it, expect, vi, beforeEach } from "vitest";

// ADAPT: Mock your auth system
// vi.mock("@/lib/supabase/server", () => ({ createClient: vi.fn() }));

const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

import { POST } from "@/app/api/feedback/route";
// import { createClient } from "@/lib/supabase/server";

function makeRequest(body: Record<string, unknown>) {
  return new Request("http://localhost/api/feedback", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
}

const validBody = {
  image: "data:image/png;base64,iVBORw0KGgo=",
  description: "Something looks wrong here",
  metadata: {
    url: "/dashboard?tab=settings",
    viewport: "1440x900",
    userAgent: "Mozilla/5.0",
  },
};

describe("POST /api/feedback", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    process.env.GITHUB_TOKEN = "ghp_test_token";
    process.env.GITHUB_FEEDBACK_REPO = "owner/repo";
  });

  // ADAPT: Add auth test cases for your auth system

  it("returns 400 when description is missing", async () => {
    const res = await POST(makeRequest({ image: validBody.image, metadata: validBody.metadata }));
    expect(res.status).toBe(400);
  });

  it("creates a GitHub issue on valid submission", async () => {
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({
        content: { download_url: "https://raw.githubusercontent.com/owner/repo/main/.feedback/img.png" },
      }), { status: 201 }),
    );
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({
        html_url: "https://github.com/owner/repo/issues/42",
        number: 42,
      }), { status: 201 }),
    );

    const res = await POST(makeRequest(validBody));
    expect(res.status).toBe(200);

    const json = await res.json();
    expect(json.issueUrl).toBe("https://github.com/owner/repo/issues/42");
    expect(json.issueNumber).toBe(42);
    expect(json.screenshotAttached).toBe(true);

    expect(mockFetch).toHaveBeenCalledTimes(2);
    const [uploadUrl, uploadOpts] = mockFetch.mock.calls[0];
    expect(uploadUrl).toContain("/repos/owner/repo/contents/.feedback/");
    expect(uploadOpts.headers.Authorization).toBe("Bearer ghp_test_token");
  });

  it("returns 502 when GitHub issue creation fails", async () => {
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({
        content: { download_url: "https://raw.githubusercontent.com/owner/repo/main/.feedback/img.png" },
      }), { status: 201 }),
    );
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({ message: "Internal Server Error" }), { status: 500 }),
    );

    const res = await POST(makeRequest(validBody));
    expect(res.status).toBe(502);
  });
});
```
