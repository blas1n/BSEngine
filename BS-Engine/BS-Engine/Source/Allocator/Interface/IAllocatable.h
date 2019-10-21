#pragma once

#include "Base/Pointer/Public/Pointer.h"

template <class T>
class IAllocatable
{
public:
	using value = T;
	using size_type = size_t;
	using difference_type = std::ptrdiff_t;
	using propagate_on_containet_move_assignment = std::true_type;
	using is_always_equal = std::true_type;

public:
	IAllocatable(size_t size) {};
	virtual ~IAllocatable() = default;

	virtual T* allocate(size_t n) = 0;
	virtual void deallocate(T* p, std::size_t n) = 0;
};