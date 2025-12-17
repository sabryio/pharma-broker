"""
Shared configuration for all PharmaBroker data analysis scripts.
"""
import os
from dotenv import load_dotenv

# Load .env file if present
load_dotenv()

# Database connection
# Supports both DATABASE_URL and PB_DATABASE_DSN formats
_dsn = os.getenv('DATABASE_URL') or os.getenv(
    'PB_DATABASE_DSN', 
    'postgres://postgres:password@localhost:5432/pharmabroker?sslmode=disable'
)
# SQLAlchemy needs postgresql:// prefix, convert postgres:// if needed
DATABASE_URL = _dsn.replace('postgres://', 'postgresql://', 1) if _dsn.startswith('postgres://') else _dsn

# Output directory
REPORTS_DIR = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'reports')
os.makedirs(REPORTS_DIR, exist_ok=True)

# Expected nullable fields by table (for null analysis)
EXPECTED_NULLS = {
    'raw_messages': [
        'sender_name', 'reply_to_id', 'reply_to_content', 'reply_to_sender',
        'processed_at', 'error', 'external_id'
    ],
    'offers': [
        'raw_message_id', 'source_name', 'group_name', 'unit', 'price',
        'expiry_date', 'batch_number', 'notes'
    ],
    'requests': [
        'raw_message_id', 'source_name', 'group_name', 'unit', 'max_price', 'notes'
    ],
    'matches': ['reasoning', 'matched_by', 'confirmed_at', 'notes'],
    'groups': ['description', 'last_message'],
    'match_feedback': ['operator_id', 'reason', 'original_confidence'],
    'review_queue': [
        'reply_context', 'failure_reason', 'reviewed_by', 'reviewed_at',
        'review_note', 'corrected_items'
    ],
    'unmapped_medications': ['approved_name', 'reviewed_at'],
    'feedback_records': ['user_id'],
    'weight_history': ['improvement', 'notes', 'performance_metrics'],
    'audit_logs': ['entity_id', 'old_value', 'new_value', 'details', 'ip_address'],
    'medication_mappings': ['embedding']
}

# Valid status values
VALID_STATUSES = {
    'offers': ['ACTIVE', 'MATCHED', 'EXPIRED', 'ARCHIVED'],
    'requests': ['ACTIVE', 'MATCHED', 'EXPIRED', 'ARCHIVED'],
    'matches': ['PENDING', 'CONFIRMED', 'REJECTED'],
    'review_queue': ['PENDING', 'APPROVED', 'REJECTED'],
}

# Foreign key relationships (child_table, child_col, parent_table, parent_col)
FK_RELATIONSHIPS = [
    ('offers', 'raw_message_id', 'raw_messages', 'id'),
    ('requests', 'raw_message_id', 'raw_messages', 'id'),
    ('matches', 'offer_id', 'offers', 'id'),
    ('matches', 'request_id', 'requests', 'id'),
    ('match_feedback', 'match_id', 'matches', 'id'),
    ('feedback_records', 'match_id', 'matches', 'id'),
    ('review_queue', 'raw_message_id', 'raw_messages', 'id'),
]
