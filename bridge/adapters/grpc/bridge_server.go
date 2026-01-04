package grpcadapter

import (
	"context"
	"fmt"

	"github.com/rs/zerolog"
	"google.golang.org/grpc"
	"google.golang.org/protobuf/proto"

	"pharma-bridge/domain"
	"pharma-bridge/ports"
	pb "pharma-bridge/proto"
)

// BridgeServer implements pb.PharmaBridgeServer.
type BridgeServer struct {
	pb.UnimplementedPharmaBridgeServer
	provider    ports.MessageProvider
	operatorJID domain.JID
	logger      zerolog.Logger
}

// NewBridgeServer creates a new PharmaBridge gRPC server.
func NewBridgeServer(provider ports.MessageProvider, operatorJID string, logger zerolog.Logger) *BridgeServer {
	return &BridgeServer{
		provider:    provider,
		operatorJID: domain.JID(operatorJID),
		logger:      logger.With().Str("component", "bridge_grpc_server").Logger(),
	}
}

// Register registers the server with the gRPC server.
func (s *BridgeServer) Register(server *grpc.Server) {
	pb.RegisterPharmaBridgeServer(server, s)
}

// ConnectMatch sends contact cards for the matched parties to the operator.
func (s *BridgeServer) ConnectMatch(ctx context.Context, req *pb.ConnectMatchRequest) (*pb.ConnectMatchResponse, error) {
	if s.operatorJID == "" {
		return &pb.ConnectMatchResponse{
			Success: false,
			Error:   "Operator JID is not configured",
		}, nil
	}

	s.logger.Info().
		Str("match_id", req.MatchId).
		Str("medication", req.Medication).
		Msg("🤝 Processing ConnectMatch request")

	// 1. Send introductory message to operator
	intro := fmt.Sprintf("🤝 *New Match Confirmed!*\n\n💊 Medication: *%s*\n🆔 Match ID: %s\n\nI'm sending you the contact cards for both parties below.",
		req.Medication, req.MatchId)

	if err := s.provider.SendMessage(ctx, s.operatorJID, intro); err != nil {
		return nil, fmt.Errorf("failed to send intro to operator: %w", err)
	}

	// 2. Send Offerer card
	offererName := fmt.Sprintf("%s (Seller - %s)", req.OffererName, req.Medication)
	if err := s.provider.SendContactCard(ctx, s.operatorJID, domain.JID(req.OffererJid), offererName, req.OffererPhone); err != nil {
		return nil, fmt.Errorf("failed to send offerer card: %w", err)
	}

	// 3. Send Requester card
	requesterName := fmt.Sprintf("%s (Buyer - %s)", req.RequesterName, req.Medication)
	if err := s.provider.SendContactCard(ctx, s.operatorJID, domain.JID(req.RequesterJid), requesterName, req.RequesterPhone); err != nil {
		return nil, fmt.Errorf("failed to send requester card: %w", err)
	}

	s.logger.Info().
		Str("match_id", req.MatchId).
		Msg("✅ Contact cards delivered to operator")

	return &pb.ConnectMatchResponse{
		Success: true,
	}, nil
}

// SendMessage sends a WhatsApp message to a recipient.
// Implements pb.PharmaBridgeServer.SendMessage
func (s *BridgeServer) SendMessage(ctx context.Context, req *pb.SendMessageRequest) (*pb.SendMessageResponse, error) {
	// Log the incoming request
	s.logger.Info().
		Str("recipient_jid", req.RecipientJid).
		Int("content_len", len(req.Content)).
		Str("reference_id", req.GetReferenceId()).
		Msg("📤 Processing SendMessage request")

	// Validate JID format
	jid, err := domain.ParseJID(req.RecipientJid)
	if err != nil {
		s.logger.Warn().
			Err(err).
			Str("recipient_jid", req.RecipientJid).
			Msg("❌ Invalid JID format")
		return &pb.SendMessageResponse{
			Success: false,
			Error:   proto.String(err.Error()),
		}, nil
	}

	// Validate content is not empty
	if req.Content == "" {
		s.logger.Warn().Msg("❌ Empty message content")
		return &pb.SendMessageResponse{
			Success: false,
			Error:   proto.String("message content cannot be empty"),
		}, nil
	}

	// Send the message via WhatsApp
	if err := s.provider.SendMessage(ctx, jid, req.Content); err != nil {
		s.logger.Error().
			Err(err).
			Str("recipient_jid", req.RecipientJid).
			Msg("❌ Failed to send message")
		return &pb.SendMessageResponse{
			Success: false,
			Error:   proto.String(fmt.Sprintf("failed to send message: %v", err)),
		}, nil
	}

	// Generate a message ID (using reference_id if provided, otherwise generate one)
	messageID := req.GetReferenceId()
	if messageID == "" {
		messageID = fmt.Sprintf("msg_%d", ctx.Value("request_id"))
		if messageID == "msg_<nil>" {
			messageID = fmt.Sprintf("msg_%d", domain.UnixTimestamp(0).Int64())
		}
	}

	s.logger.Info().
		Str("recipient_jid", req.RecipientJid).
		Str("message_id", messageID).
		Msg("✅ Message sent successfully")

	return &pb.SendMessageResponse{
		Success:   true,
		MessageId: messageID,
	}, nil
}
