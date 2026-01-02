// Pipeline Recording Types
// Type definitions for internal backend pipeline stages

// ============================================================================
// Pipeline Stage Types
// ============================================================================

export type PipelineStage =
  | 'message_received'
  | 'ai_parsing'
  | 'parsing_complete'
  | 'medication_resolution'
  | 'offer_created'
  | 'request_created'
  | 'match_candidate_search'
  | 'hierarchical_stage_1'
  | 'hierarchical_stage_2'
  | 'hierarchical_stage_3'
  | 'score_calculation'
  | 'ai_review'
  | 'consensus_check'
  | 'contrastive_validation'
  | 'calibration'
  | 'match_created'
  | 'queue_added'
  | 'notification_sent'

export type PipelineStepStatus =
  | 'pending'
  | 'running'
  | 'success'
  | 'error'
  | 'skipped'

// ============================================================================
// Pipeline Step
// ============================================================================

export interface PipelineStep {
  id: string
  stage: PipelineStage
  status: PipelineStepStatus
  startedAt: string
  completedAt?: string
  durationMs?: number
  input?: Record<string, unknown>
  output?: Record<string, unknown>
  error?: string
  metadata?: Record<string, unknown>
}

// ============================================================================
// Parsing Details
// ============================================================================

export interface ParsingDetails {
  rawMessage: string
  language: string
  detectedType: 'offer' | 'request'
  aiModel: string
  temperature: number
  promptTokens: number
  completionTokens: number
  parsedFields: {
    medication: string
    medicationRaw: string
    quantity: number
    unit: string
    price: number | null
    currency: string | null
    expiryDate: string | null
    urgencyLevel: string | null
  }
  confidence: number
  alternatives?: { medication: string; confidence: number }[]
}

// ============================================================================
// Resolution Details
// ============================================================================

export interface ResolutionStage {
  stage: 'exact_alias' | 'exact_name' | 'fuzzy' | 'semantic'
  attempted: boolean
  matched: boolean
  result?: {
    masterId: string
    masterName: string
    score: number
  }
  durationMs: number
}

export interface ResolutionDetails {
  inputText: string
  stages: ResolutionStage[]
  finalResult: {
    masterId: string
    masterName: string
    matchType: string
    confidence: number
  } | null
}

// ============================================================================
// Hierarchical Matching Stage
// ============================================================================

export interface HierarchicalStage {
  stageName: string
  stageNumber: number
  candidatesIn: number
  candidatesOut: number
  threshold: number
  scores: {
    candidateId: string
    candidateName: string
    score: number
    passed: boolean
  }[]
  durationMs: number
}

// ============================================================================
// Score Breakdown
// ============================================================================

export interface PipelineScoreBreakdown {
  medicationScore: number
  medicationWeight: number
  rawTextScore: number
  rawTextWeight: number
  embeddingScore: number | null
  embeddingWeight: number
  dosageScore: number
  dosageWeight: number
  quantityScore: number
  quantityWeight: number
  priceScore: number | null
  priceWeight: number
  recencyScore: number
  recencyWeight: number
  aiLogicScore: number | null
  aiLogicWeight: number
  finalScore: number
  formula: string
}

// ============================================================================
// AI Review Details
// ============================================================================

export interface AIReviewDetails {
  model: string
  temperature: number
  promptTokens: number
  completionTokens: number
  offerText: string
  requestText: string
  decision: 'approve' | 'reject' | 'uncertain'
  confidence: number
  explanation: string
  reasoning: string[]
  issues: string[]
  durationMs: number
}

// ============================================================================
// Consensus Details
// ============================================================================

export interface ConsensusDetails {
  auditors: {
    name: string
    decision: 'approve' | 'reject' | 'uncertain'
    confidence: number
    weight: number
  }[]
  finalDecision: 'approve' | 'reject' | 'uncertain'
  agreementScore: number
  threshold: number
}

// ============================================================================
// Contrastive Validation
// ============================================================================

export interface ContrastiveDetails {
  targetScore: number
  negativeScores: {
    candidateId: string
    candidateName: string
    score: number
  }[]
  averageNegativeScore: number
  maxNegativeScore: number
  marginVsAverage: number
  marginVsMax: number
  marginThresholdAvg: number
  marginThresholdMax: number
  passed: boolean
}

// ============================================================================
// Calibration Details
// ============================================================================

export interface CalibrationDetails {
  rawScore: number
  calibratedScore: number
  calibrationMethod: string
  binIndex?: number
  binCount?: number
  ece?: number
  mce?: number
}

// ============================================================================
// Pipeline Recording
// ============================================================================

export interface PipelineRecording {
  id: string
  matchId: string
  offerId: string
  requestId: string
  startedAt: string
  completedAt?: string
  totalDurationMs?: number
  status: 'running' | 'completed' | 'error'
  steps: PipelineStep[]
  parsing?: {
    offer: ParsingDetails
    request: ParsingDetails
  }
  resolution?: {
    offer: ResolutionDetails
    request: ResolutionDetails
  }
  hierarchicalStages?: HierarchicalStage[]
  scoreBreakdown?: PipelineScoreBreakdown
  aiReview?: AIReviewDetails
  consensus?: ConsensusDetails
  contrastive?: ContrastiveDetails
  calibration?: CalibrationDetails
  finalScore?: number
  finalStatus?: 'auto_approved' | 'needs_review' | 'auto_rejected'
}

// ============================================================================
// Stage Colors and Icons
// ============================================================================

export const STAGE_COLORS: Record<
  PipelineStage,
  { bg: string; text: string; icon: string }
> = {
  message_received: { bg: 'bg-blue-500/20', text: 'text-blue-400', icon: '📨' },
  ai_parsing: { bg: 'bg-purple-500/20', text: 'text-purple-400', icon: '🤖' },
  parsing_complete: {
    bg: 'bg-violet-500/20',
    text: 'text-violet-400',
    icon: '✨',
  },
  medication_resolution: {
    bg: 'bg-cyan-500/20',
    text: 'text-cyan-400',
    icon: '💊',
  },
  offer_created: {
    bg: 'bg-emerald-500/20',
    text: 'text-emerald-400',
    icon: '📤',
  },
  request_created: { bg: 'bg-teal-500/20', text: 'text-teal-400', icon: '📥' },
  match_candidate_search: {
    bg: 'bg-amber-500/20',
    text: 'text-amber-400',
    icon: '🔍',
  },
  hierarchical_stage_1: {
    bg: 'bg-orange-500/20',
    text: 'text-orange-400',
    icon: '1️⃣',
  },
  hierarchical_stage_2: {
    bg: 'bg-orange-500/20',
    text: 'text-orange-400',
    icon: '2️⃣',
  },
  hierarchical_stage_3: {
    bg: 'bg-orange-500/20',
    text: 'text-orange-400',
    icon: '3️⃣',
  },
  score_calculation: {
    bg: 'bg-pink-500/20',
    text: 'text-pink-400',
    icon: '📊',
  },
  ai_review: { bg: 'bg-indigo-500/20', text: 'text-indigo-400', icon: '🧠' },
  consensus_check: { bg: 'bg-lime-500/20', text: 'text-lime-400', icon: '🤝' },
  contrastive_validation: {
    bg: 'bg-rose-500/20',
    text: 'text-rose-400',
    icon: '⚖️',
  },
  calibration: { bg: 'bg-sky-500/20', text: 'text-sky-400', icon: '🎯' },
  match_created: { bg: 'bg-green-500/20', text: 'text-green-400', icon: '✅' },
  queue_added: { bg: 'bg-slate-500/20', text: 'text-slate-400', icon: '📋' },
  notification_sent: {
    bg: 'bg-fuchsia-500/20',
    text: 'text-fuchsia-400',
    icon: '🔔',
  },
}

export const STATUS_COLORS: Record<
  PipelineStepStatus,
  { bg: string; text: string }
> = {
  pending: { bg: 'bg-slate-500/20', text: 'text-slate-400' },
  running: { bg: 'bg-amber-500/20', text: 'text-amber-400' },
  success: { bg: 'bg-emerald-500/20', text: 'text-emerald-400' },
  error: { bg: 'bg-red-500/20', text: 'text-red-400' },
  skipped: { bg: 'bg-gray-500/20', text: 'text-gray-400' },
}
