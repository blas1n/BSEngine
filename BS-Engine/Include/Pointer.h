#pragma once

#include "Core.h"
#include <ostream>
#include <utility>
#include <variant>

/**
 * @brief
 * Custom pointer class that can be used like a normal pointer
 * @detail
 * It is implemented for pointer stability.
 * Indirectly accessed via handle.
 * @todo Access with handle.
*/
template <class T>
class Pointer {
public:
	constexpr Pointer() noexcept = default;

	constexpr Pointer(std::nullptr_t) noexcept
		: handle(0) {}

	Pointer(const uint32 inHandle) noexcept
		: handle(inHandle) {}

	template <class U>
	Pointer(const Pointer<U>& other) noexcept
		: handle(other.handle) {}

	template <class U>
	Pointer(Pointer<U>&& other) noexcept
		: handle(std::move(other.handle)) {}

	Pointer(T* ptr) noexcept
		: handle(ptr) {}

	~Pointer() = default;

	template <class U>
	Pointer& operator=(const Pointer<U>& other) noexcept
	{
		handle = other.handle;
		return *this;
	}

	template <class U>
	Pointer& operator=(Pointer<U>&& other) noexcept
	{
		handle = std::move(other.handle);
		return *this;
	}

	T& operator*() const noexcept {
		return *Get();
	}

	T* operator->() const noexcept {
		return Get();
	}

private:
	T* Get() const noexcept {
		auto ptr = std::get_if<T*>(&handle);
		if (!ptr)
			ptr = *std::get<uint32>(handle);
		
		return ptr;
	}

	std::variant<uint32, T*> handle;
};

template <class T, class U>
bool operator==(const Pointer<T>& lhs, const Pointer<U>& rhs) noexcept
{
	return lhs.handle == rhs.handle;
}

template <class T, class U>
bool operator!=(const Pointer<T>& lhs, const Pointer<U>& rhs) noexcept
{
	return lhs.handle != lhs.handle;
}

template <class T, class U>
bool operator>(const Pointer<T>& lhs, const Pointer<U>& rhs) noexcept
{
	return lhs.handle > lhs.handle;
}

template <class T, class U>
bool operator>=(const Pointer<T>& lhs, const Pointer<U>& rhs) noexcept
{
	return lhs.handle >= lhs.handle;
}

template <class T, class U>
bool operator<(const Pointer<T>& lhs, const Pointer<U>& rhs) noexcept
{
	return lhs.handle < lhs.handle;
}

template <class T, class U>
bool operator<=(const Pointer<T>& lhs, const Pointer<U>& rhs) noexcept
{
	return lhs.handle <= lhs.handle;
}

template <class T, class U, class V>
std::basic_ostream<U, V>& operator<<(std::basic_ostream<U, V>& os, const Pointer<T>& ptr) {
	os << ptr.Get();
	return os;
}