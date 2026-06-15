// Package ids provides stable ID allocation utilities.
package ids

import (
	"sync"
)

// Allocator maintains sequential, duplicate-free ID allocation.
type Allocator struct {
	mu       sync.Mutex
	nextID   uint64
	reserved map[uint64]bool
}

// NewAllocator creates a new ID allocator starting from ID 0.
func NewAllocator() *Allocator {
	return &Allocator{
		nextID:   0,
		reserved: make(map[uint64]bool),
	}
}

// NewAllocatorFromID creates a new ID allocator starting from a specific ID.
func NewAllocatorFromID(startID uint64) *Allocator {
	return &Allocator{
		nextID:   startID,
		reserved: make(map[uint64]bool),
	}
}

// AllocateID returns the next sequential ID.
func (a *Allocator) AllocateID() uint64 {
	a.mu.Lock()
	defer a.mu.Unlock()

	id := a.nextID
	a.nextID++
	a.reserved[id] = true
	return id
}

// AllocateN allocates N sequential IDs and returns them as a slice.
func (a *Allocator) AllocateN(n int) []uint64 {
	if n <= 0 {
		return []uint64{}
	}

	a.mu.Lock()
	defer a.mu.Unlock()

	ids := make([]uint64, n)
	for i := 0; i < n; i++ {
		ids[i] = a.nextID
		a.reserved[a.nextID] = true
		a.nextID++
	}
	return ids
}

// IsAllocated checks if an ID has been allocated.
func (a *Allocator) IsAllocated(id uint64) bool {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.reserved[id]
}

// Next returns the ID of the next allocation without allocating it.
func (a *Allocator) Next() uint64 {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.nextID
}

// Reserve pre-reserves an ID to prevent it from being allocated.
// Returns true if the ID was successfully reserved, false if it was already reserved.
func (a *Allocator) Reserve(id uint64) bool {
	a.mu.Lock()
	defer a.mu.Unlock()

	if a.reserved[id] {
		return false
	}
	a.reserved[id] = true
	return true
}

// Reset resets the allocator to start from ID 0.
func (a *Allocator) Reset() {
	a.mu.Lock()
	defer a.mu.Unlock()
	a.nextID = 0
	a.reserved = make(map[uint64]bool)
}

// ResetWithID resets the allocator to start from a specific ID.
func (a *Allocator) ResetWithID(startID uint64) {
	a.mu.Lock()
	defer a.mu.Unlock()
	a.nextID = startID
	a.reserved = make(map[uint64]bool)
}

// Count returns the number of IDs that have been allocated.
func (a *Allocator) Count() int {
	a.mu.Lock()
	defer a.mu.Unlock()
	return len(a.reserved)
}

// Global allocator for convenience.
var globalAllocator = NewAllocator()

// AllocateID allocates a new ID from the global allocator.
func AllocateID() uint64 {
	return globalAllocator.AllocateID()
}

// AllocateN allocates N sequential IDs from the global allocator.
func AllocateN(n int) []uint64 {
	return globalAllocator.AllocateN(n)
}

// IsAllocated checks if an ID has been allocated in the global allocator.
func IsAllocated(id uint64) bool {
	return globalAllocator.IsAllocated(id)
}

// NextID returns the ID of the next allocation from the global allocator.
func NextID() uint64 {
	return globalAllocator.Next()
}

// ResetGlobal resets the global allocator.
func ResetGlobal() {
	globalAllocator.Reset()
}
