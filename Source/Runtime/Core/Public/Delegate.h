#pragma once

#include "BSBase/Type.h"
#include "DelegateInst.h"

template <class T>
class Delegate;

template <class R, class... Args>
class Delegate<R(Args...)> final
{
public:
	Delegate() noexcept : size(0) {}

	Delegate(std::nullptr_t) noexcept : Delegate() {}

	Delegate(const Delegate& other) noexcept : Delegate()
	{
		if (const auto* inst = other.GetInst())
		{
			size = other.size;
			if (size > sizeof(storage))
			{
				storage[0] = malloc(size);
				memcpy(storage[0], inst, size);
			}
			else
			{
				memcpy(storage, inst, size);
			}
		}
	}

	Delegate(Delegate&& other) noexcept : Delegate()
	{
		memcpy(this, &other, sizeof(*this));
		memset(&other, 0, sizeof(other));
	}

	Delegate& operator=(const Delegate& other) noexcept
	{
		if (*this == other) return *this;

		Clear();

		if (const auto* inst = other.GetInst())
		{
			size = other.size;
			if (size > sizeof(storage))
			{
				storage[0] = malloc(size);
				memcpy(storage[0], inst, size);
			}
			else
			{
				memcpy(storage, inst, size);
			}
		}

		return *this;
	}

	Delegate& operator=(Delegate&& other) noexcept
	{
		if (*this == other) return *this;

		Clear();
		
		memcpy(this, &other, sizeof(*this));
		memset(&other, 0, sizeof(other));
		return *this;
	}

	Delegate(R(*fn)(Args...)) : Delegate()
	{
		CreateInstance<Impl::DelegateInstFunction<R, Args...>>(fn);
	}

	template <class T>
	Delegate(T* obj, R(T::* fn)(Args...)) : Delegate()
	{
		CreateInstance<Impl::DelegateInstMethod<T, R, Args...>>(obj, fn);
	}

	template <class T>
	Delegate(T* obj, R(T::* fn)(Args...) const) : Delegate()
	{
		CreateInstance<Impl::DelegateInstConstMethod<T, R, Args...>>(obj, fn);
	}

	template <class Func>
	Delegate(Func&& fn) : Delegate()
	{
		CreateInstance<Impl::DelegateInstFunctor<Func, R, Args...>>(std::forward<Func>(fn));
	}

	~Delegate() { Clear(); }

	R operator()(const Args&... args) const
	{
		return IsBound() ? GetInst()->Execute(args...) : R();
	}

	void Clear() noexcept
	{
		if (const auto heap = GetHeap())
			free(heap);

		memset(this, 0, sizeof(*this));
	}

	[[nodiscard]] bool IsBound() const noexcept { return size; }
	[[nodiscard]] operator bool() const noexcept { return IsBound(); }

	[[nodiscard]] friend bool operator==(const Delegate& lhs, const Delegate& rhs)
	{
		if (lhs.size != rhs.size)
			return false;

		const auto lhsInst = lhs.GetInst();
		const auto rhsInst = rhs.GetInst();

		if (!lhsInst) return !rhsInst;
		if (!rhsInst) return false;

		return !memcmp(lhsInst, rhsInst, lhs.size);
	}

private:
	template <class T, class... Args>
	void CreateInstance(Args&&... args)
	{
		if constexpr (sizeof(T) > sizeof(storage))
		{
			storage[0] = malloc(sizeof(T));
			new(storage[0]) T{ std::forward<Args>(args)... };
		}
		else
			new(storage) T{ std::forward<Args>(args)... };

		size = sizeof(T);
	}

	[[nodiscard]] Impl::DelegateInstBase<R, Args...>* GetInst() noexcept
	{
		return const_cast<Impl::DelegateInstBase<R, Args...>*>(static_cast<const Delegate*>(this)->GetInst());
	}

	[[nodiscard]] const Impl::DelegateInstBase<R, Args...>* GetInst() const noexcept
	{
		if (!size) return nullptr;

		if (const auto heap = GetHeap())
			return reinterpret_cast<const Impl::DelegateInstBase<R, Args...>*>(heap);
		
		return reinterpret_cast<const Impl::DelegateInstBase<R, Args...>*>(&storage);
	}

	[[nodiscard]] void* GetHeap() noexcept
	{
		return (size > sizeof(storage)) ? storage[0] : nullptr;
	}

	[[nodiscard]] const void* GetHeap() const noexcept
	{
		return (size > sizeof(storage)) ? storage[0] : nullptr;
	}

private:
	constexpr static auto StorageSize = 3;
	void* storage[StorageSize];
	BSBase::uint32 size;
};

template <class T>
[[nodiscard]] bool operator!=(const Delegate<T>& lhs, const Delegate<T>& rhs)
{
	return !(lhs == rhs);
}
