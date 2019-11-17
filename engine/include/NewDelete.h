#include "Allocator.h"
#include "Type.h"

// To disable inconsistent annotation. (My new is noexcept)
#pragma warning (disable:28251)

void* operator new  (size_t count) noexcept;
void* operator new[](size_t count) noexcept;

void operator delete  (void* ptr) noexcept;
void operator delete[](void* ptr) noexcept;

void operator delete  (void* ptr, std::size_t sz) noexcept;
void operator delete[](void* ptr, std::size_t sz) noexcept;