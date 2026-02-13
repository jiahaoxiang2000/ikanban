import type { Part, Session, SessionMessagesResponses } from "@opencode-ai/sdk/v2/client";

export type ConversationSessionMeta = {
  sessionID: string;
  projectId: string;
  taskId: string;
  directory: string;
  title?: string;
  createdAt: number;
  updatedAt: number;
  lastMessageAt?: number;
};

type SdkMessageInfoCore = Pick<ConversationSdkSessionMessage["info"], "id" | "sessionID" | "role">;

export type ConversationSdkSession = Session;
export type ConversationSdkPart = Part;
export type ConversationSdkSessionMessage = SessionMessagesResponses[200][number];
export type ConversationMessageMeta = SdkMessageInfoCore;

export function updateSessionActivity(
  session: ConversationSessionMeta,
  at: number = Date.now(),
): ConversationSessionMeta {
  return {
    ...session,
    updatedAt: at,
    lastMessageAt: at,
  };
}
