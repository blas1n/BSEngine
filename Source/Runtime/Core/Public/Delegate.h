#pragma once

#include <utility>
#include "DelegateInst.h"

template <class R, class... Args>
class Delegate final
{
public:
	Delegate() noexcept : storage(nullptr) {}

	Delegate(std::nullptr_t) noexcept : Delegate() {}

	Delegate(const Delegate& other) noexcept
	{
		if (other.storage)
			other.GetInst()->CloneTo(storage);
	}

	Delegate(Delegate&& other) noexcept
	{
		storage = other.storage;
		other.Clear();
	}

	Delegate& operator=(const Delegate& other) noexcept
	{
		if (*this == other) return *this;

		Clear();

		if (other.storage)
			other.GetInst()->CloneTo(storage);

		return *this;
	}

	Delegate& operator=(Delegate&& other) noexcept
	{
		if (*this == other) return *this;

		Clear();

		storage = std::move(other.storage);
		return *this;
	}

	Delegate(R(*fn)(Args...))
		: storage(Impl::DelegateInstFunction<R, Args...>::Create(fn)) {}

	template <class T>
	Delegate(T* obj, Impl::MethodPtrType<T, R, Args...> fn)
		: storage(Impl::DelegateInstMethod<T, R, Args...>::Create(obj, fn)) {}

	template <class Func>
	Delegate(Func&& fn)
		: storage(Impl::DelegateInstFunctor<Func, R, Args...>::Create(std::forward<Func>(fn))) {}

	~Delegate() { Clear(); }

	R operator()(const Args&... args)
	{
		return IsBound() ? GetInst()->Execute(args...) : R();
	}

	R operator()(Args&&... args)
	{
		return IsBound() ? GetInst()->Execute(std::move(args)...) : R();
	}

	void Clear() noexcept
	{
		if (const auto heap = storage.GetHeap())
			delete heap;
		
		storage = nullptr;
	}

	[[nodiscard]] bool IsBound() const noexcept { return storage.IsBound(); }
	[[nodiscard]] operator bool() const noexcept { return IsBound(); }

private:
	[[nodiscard]] Impl::DelegateInstBase<R, Args...>* GetInst() noexcept
	{
		if (const auto heap = storage.GetHeap())
			return reinterpret_cast<Impl::DelegateInstBase<R, Args...>*>(heap);
		
		return reinterpret_cast<Impl::DelegateInstBase<R, Args...>*>(&storage);
	}

private:
	Impl::DelegateStorage storage;
};

template <class R, class... Args>
[[nodiscard]] bool operator==(const Delegate<R, Args...>& lhs, const Delegate<R, Args...>& rhs)
{
	return &lhs == &rhs;
}

template <class R, class... Args>
[[nodiscard]] bool operator!=(const Delegate<R, Args...>& lhs, const Delegate<R, Args...>& rhs)
{
	return !(lhs == rhs);
}
