#include "pch.h"
#include "PoolMemory.h"

TEST(PoolMemory, Create)
{
	PoolMemory<1> pool{ 10 };
	ASSERT_EQ(pool.GetAssignedByte(), 0);
	ASSERT_EQ(pool.GetAssignableByte(), 10);
	ASSERT_EQ(pool.GetMaxByte(), 10);
}

TEST(PoolMemory, Malloc)
{
	PoolMemory<sizeof(int)> pool{ 5 };
	auto p = pool.Malloc(6);
	ASSERT_EQ(p, nullptr);

	p = pool.Malloc(3);
	ASSERT_EQ(pool.GetAssignableByte(), 2 * sizeof(int));

	p = pool.Malloc(3);
	ASSERT_EQ(p, nullptr);

	p = pool.Malloc(2);
	ASSERT_EQ(pool.GetAssignableByte(), 0);
}

TEST(PoolMemory, Free)
{
	PoolMemory<sizeof(int)> pool{ 5 };

	auto p = pool.Malloc(5);
	pool.Free(p);
	ASSERT_EQ(pool.GetAssignedByte(), 0);

	auto a = pool.Malloc(2);
	auto b = pool.Malloc(1);
	auto c = pool.Malloc(2);

	pool.Free(b);
	ASSERT_EQ(pool.GetAssignableByte(), sizeof(int));

	pool.Free(c);
	ASSERT_EQ(pool.GetAssignedByte(), 2 * sizeof(int));

	pool.Free(a);
	ASSERT_EQ(pool.GetAssignedByte(), 0);	
}

TEST(PoolMemory, FreeInvalidArgument)
{
	PoolMemory<sizeof(int)> pool{ 5 };

	auto p = static_cast<unsigned char*>(pool.Malloc(1));
	ASSERT_DEBUG_DEATH(pool.Free(p - 1), ".*");
	ASSERT_DEBUG_DEATH(pool.Free(p + 1), ".*");
	ASSERT_DEBUG_DEATH(pool.Free(nullptr), ".*");
}