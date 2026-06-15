package ids

import (
	"sync"
	"testing"
)

func TestAllocatorSequential(t *testing.T) {
	a := NewAllocator()

	// Allocate some IDs and verify they are sequential
	id1 := a.AllocateID()
	id2 := a.AllocateID()
	id3 := a.AllocateID()

	if id1 != 0 {
		t.Errorf("Expected first ID to be 0, got %d", id1)
	}
	if id2 != 1 {
		t.Errorf("Expected second ID to be 1, got %d", id2)
	}
	if id3 != 2 {
		t.Errorf("Expected third ID to be 2, got %d", id3)
	}
}

func TestAllocatorNoDuplicates(t *testing.T) {
	a := NewAllocator()
	allocated := make(map[uint64]bool)

	// Allocate 100 IDs and verify no duplicates
	for i := 0; i < 100; i++ {
		id := a.AllocateID()
		if allocated[id] {
			t.Errorf("Duplicate ID allocated: %d", id)
		}
		allocated[id] = true
	}

	if len(allocated) != 100 {
		t.Errorf("Expected 100 unique IDs, got %d", len(allocated))
	}
}

func TestAllocateN(t *testing.T) {
	a := NewAllocator()

	ids := a.AllocateN(5)
	if len(ids) != 5 {
		t.Errorf("Expected 5 IDs, got %d", len(ids))
	}

	for i, id := range ids {
		if id != uint64(i) {
			t.Errorf("ID at index %d should be %d, got %d", i, i, id)
		}
	}

	// Allocate more and verify continuation
	nextID := a.AllocateID()
	if nextID != 5 {
		t.Errorf("Expected next ID to be 5, got %d", nextID)
	}
}

func TestAllocateNZero(t *testing.T) {
	a := NewAllocator()

	ids := a.AllocateN(0)
	if len(ids) != 0 {
		t.Errorf("Expected empty slice for AllocateN(0), got %d IDs", len(ids))
	}

	ids = a.AllocateN(-1)
	if len(ids) != 0 {
		t.Errorf("Expected empty slice for AllocateN(-1), got %d IDs", len(ids))
	}
}

func TestIsAllocated(t *testing.T) {
	a := NewAllocator()

	id1 := a.AllocateID()
	id2 := a.AllocateID()

	if !a.IsAllocated(id1) {
		t.Errorf("ID %d should be allocated", id1)
	}
	if !a.IsAllocated(id2) {
		t.Errorf("ID %d should be allocated", id2)
	}
	if a.IsAllocated(99) {
		t.Errorf("ID 99 should not be allocated")
	}
}

func TestNewAllocatorFromID(t *testing.T) {
	a := NewAllocatorFromID(100)

	id1 := a.AllocateID()
	id2 := a.AllocateID()

	if id1 != 100 {
		t.Errorf("Expected first ID to be 100, got %d", id1)
	}
	if id2 != 101 {
		t.Errorf("Expected second ID to be 101, got %d", id2)
	}
}

func TestNext(t *testing.T) {
	a := NewAllocator()

	next1 := a.Next()
	if next1 != 0 {
		t.Errorf("Expected next ID to be 0, got %d", next1)
	}

	a.AllocateID()
	a.AllocateID()

	next2 := a.Next()
	if next2 != 2 {
		t.Errorf("Expected next ID to be 2, got %d", next2)
	}
}

func TestReserve(t *testing.T) {
	a := NewAllocator()

	// Reserve an ID that hasn't been allocated yet
	if !a.Reserve(5) {
		t.Errorf("Failed to reserve ID 5")
	}

	// Try to reserve the same ID again
	if a.Reserve(5) {
		t.Errorf("Should not be able to reserve ID 5 twice")
	}

	// Verify it's marked as allocated
	if !a.IsAllocated(5) {
		t.Errorf("Reserved ID 5 should be marked as allocated")
	}
}

func TestReset(t *testing.T) {
	a := NewAllocator()

	a.AllocateID()
	a.AllocateID()
	a.AllocateID()

	a.Reset()

	// After reset, next ID should be 0
	nextID := a.Next()
	if nextID != 0 {
		t.Errorf("Expected next ID to be 0 after reset, got %d", nextID)
	}

	// Allocate IDs again and verify they start from 0
	id1 := a.AllocateID()
	if id1 != 0 {
		t.Errorf("Expected first ID after reset to be 0, got %d", id1)
	}
}

func TestResetWithID(t *testing.T) {
	a := NewAllocator()

	a.AllocateID()
	a.AllocateID()

	a.ResetWithID(50)

	nextID := a.Next()
	if nextID != 50 {
		t.Errorf("Expected next ID to be 50 after reset, got %d", nextID)
	}

	id1 := a.AllocateID()
	if id1 != 50 {
		t.Errorf("Expected first ID after reset to be 50, got %d", id1)
	}
}

func TestCount(t *testing.T) {
	a := NewAllocator()

	if a.Count() != 0 {
		t.Errorf("Expected count to be 0, got %d", a.Count())
	}

	a.AllocateID()
	a.AllocateID()
	a.AllocateID()

	if a.Count() != 3 {
		t.Errorf("Expected count to be 3, got %d", a.Count())
	}
}

func TestConcurrency(t *testing.T) {
	a := NewAllocator()
	numGoroutines := 10
	idsPerGoroutine := 100

	var wg sync.WaitGroup
	results := make(chan []uint64, numGoroutines)

	for i := 0; i < numGoroutines; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			ids := a.AllocateN(idsPerGoroutine)
			results <- ids
		}()
	}

	wg.Wait()
	close(results)

	// Collect all allocated IDs and verify no duplicates
	allocated := make(map[uint64]bool)
	for ids := range results {
		for _, id := range ids {
			if allocated[id] {
				t.Errorf("Duplicate ID allocated: %d", id)
			}
			allocated[id] = true
		}
	}

	if len(allocated) != numGoroutines*idsPerGoroutine {
		t.Errorf("Expected %d unique IDs, got %d", numGoroutines*idsPerGoroutine, len(allocated))
	}
}

func TestGlobalAllocator(t *testing.T) {
	ResetGlobal()

	id1 := AllocateID()
	id2 := AllocateID()

	if id1 != 0 {
		t.Errorf("Expected first global ID to be 0, got %d", id1)
	}
	if id2 != 1 {
		t.Errorf("Expected second global ID to be 1, got %d", id2)
	}

	ids := AllocateN(3)
	if len(ids) != 3 {
		t.Errorf("Expected 3 IDs from global allocator, got %d", len(ids))
	}

	if !IsAllocated(0) {
		t.Errorf("ID 0 should be allocated in global allocator")
	}

	next := NextID()
	if next != 5 {
		t.Errorf("Expected next ID to be 5, got %d", next)
	}
}
