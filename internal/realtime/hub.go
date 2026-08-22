package realtime

import "sync"

const subscriberBufferSize = 32

type Event struct {
	ID   string
	Type string
	Data any
}

type Hub struct {
	mu          sync.RWMutex
	nextID      uint64
	subscribers map[uint]map[uint64]chan Event
}

func NewHub() *Hub {
	return &Hub{subscribers: make(map[uint]map[uint64]chan Event)}
}

func (h *Hub) Subscribe(userID uint) (<-chan Event, func()) {
	h.mu.Lock()
	h.nextID++
	subscriberID := h.nextID
	channel := make(chan Event, subscriberBufferSize)
	if h.subscribers[userID] == nil {
		h.subscribers[userID] = make(map[uint64]chan Event)
	}
	h.subscribers[userID][subscriberID] = channel
	h.mu.Unlock()

	var once sync.Once
	return channel, func() {
		once.Do(func() {
			h.mu.Lock()
			delete(h.subscribers[userID], subscriberID)
			if len(h.subscribers[userID]) == 0 {
				delete(h.subscribers, userID)
			}
			close(channel)
			h.mu.Unlock()
		})
	}
}

func (h *Hub) Publish(userID uint, event Event) {
	h.mu.RLock()
	defer h.mu.RUnlock()

	for _, channel := range h.subscribers[userID] {
		select {
		case channel <- event:
		default:
		}
	}
}
