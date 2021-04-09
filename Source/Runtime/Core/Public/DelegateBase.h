#pragma once

#include <utility>

namespace Impl
{
	class DelegateBase
	{
	public:
		DelegateBase() noexcept
			: ptr(nullptr), size(0) {}

		DelegateBase(std::nullptr_t) noexcept : DelegateBase() {}

		DelegateBase(const DelegateBase& other) noexcept;
		DelegateBase(DelegateBase&& other) noexcept;

		DelegateBase& operator=(const DelegateBase& other) noexcept;
		DelegateBase& operator=(DelegateBase&& other) noexcept;

		virtual ~DelegateBase() { Clear(); }

		void Clear() noexcept;

		[[nodiscard]] bool IsBound() const noexcept { return ptr; }
		[[nodiscard]] operator bool() const noexcept { return ptr; }

	protected:
		template <class T>
		void Allocate(T&& obj)
		{
			size = sizeof(T);

			if constexpr (sizeof(T) > sizeof(ptr))
				ptr = new T{ std::forward<T>(obj) };
			else
				memcpy(ptr, &obj, size);
		}

		const void* GetPtr() const
		{
			if (!ptr) return nullptr;

			return (size > sizeof(ptr)) ? ptr : &ptr;
		}

	private:
		void* ptr;
		size_t size;
	};
}
