package message

import (
	"bufio"
	"bytes"
	"nomail-api/internal/realtime"
	"testing"
)

func TestWriteSSEEvent(t *testing.T) {
	var output bytes.Buffer
	writer := bufio.NewWriter(&output)
	err := writeSSEEvent(writer, realtime.Event{
		ID:   "message-1",
		Type: "message.created",
		Data: struct {
			ID int `json:"id"`
		}{ID: 1},
	})
	if err != nil {
		t.Fatalf("writeSSEEvent() error = %v", err)
	}

	want := "id: message-1\nevent: message.created\ndata: {\"id\":1}\n\n"
	if output.String() != want {
		t.Fatalf("writeSSEEvent() = %q, want %q", output.String(), want)
	}
}

func TestWriteSSEEventRejectsInvalidData(t *testing.T) {
	writer := bufio.NewWriter(&bytes.Buffer{})
	if err := writeSSEEvent(writer, realtime.Event{Data: make(chan int)}); err == nil {
		t.Fatal("writeSSEEvent() expected JSON encoding error")
	}
}
