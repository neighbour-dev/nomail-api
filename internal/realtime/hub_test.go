package realtime

import (
	"testing"
	"time"
)

func TestHubPublishesToMatchingUser(t *testing.T) {
	hub := NewHub()
	userOneEvents, unsubscribeUserOne := hub.Subscribe(1)
	defer unsubscribeUserOne()
	userTwoEvents, unsubscribeUserTwo := hub.Subscribe(2)
	defer unsubscribeUserTwo()

	want := Event{ID: "message-1", Type: "message.created", Data: "payload"}
	hub.Publish(1, want)

	select {
	case got := <-userOneEvents:
		if got.ID != want.ID || got.Type != want.Type || got.Data != want.Data {
			t.Fatalf("event = %#v, want %#v", got, want)
		}
	case <-time.After(time.Second):
		t.Fatal("timed out waiting for event")
	}

	select {
	case event := <-userTwoEvents:
		t.Fatalf("unexpected event for another user: %#v", event)
	default:
	}
}

func TestHubUnsubscribeClosesChannel(t *testing.T) {
	hub := NewHub()
	events, unsubscribe := hub.Subscribe(1)
	unsubscribe()
	unsubscribe()

	if _, open := <-events; open {
		t.Fatal("subscriber channel is still open")
	}
}

func TestHubDoesNotBlockOnSlowSubscriber(t *testing.T) {
	hub := NewHub()
	_, unsubscribe := hub.Subscribe(1)
	defer unsubscribe()

	done := make(chan struct{})
	go func() {
		for i := 0; i < subscriberBufferSize*2; i++ {
			hub.Publish(1, Event{Type: "message.created"})
		}
		close(done)
	}()

	select {
	case <-done:
	case <-time.After(time.Second):
		t.Fatal("Publish blocked on a slow subscriber")
	}
}
