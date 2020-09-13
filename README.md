# Delegate

This pallet demonstrates a way to bound runtime recursion with module-level constraints. Bond calculations disincentivize large (1), wide (2), and deep (3) tree delegation.
1. group size, number of members `Vec<AccountId>` associated on-chain with `TreeId` s.t. each tree can only have up to `Trait::MaxSize` members in total
2. number of subtrees, each tree can only have up to `Trait::MaxKids` number of subtrees for delegation
3. depth, a tree can only have height up to `Trait::MaxDepth` or it is not allowed to be created for delegation, `depth_of_child = depth_of_parent + 1`

Adding new members and delegating to subtrees is disincentivized by collateral requirements. Bonds for adding new members scale linearly with group size. Bonds for adding new subtrees scales exponentially with number of children and depth.

## Rules

* Any account can register a tree with a `TreeId` and add a set of members `Vec<AccountId>`
* If the `TreeState` has `height = 0`, the account that registered the Tree is the only account that can add and remove members
* Otherwise (`height > 0`), any account in the parent tree can add or remove members (as long as new member count is leq `Trait::MaxSize`)
* Any member of the set `Vec<AccountId>` associated with the `TreeId` can delegate permissions to a new `TreeId` (as long as subtree height is leq `Trait::MaxDepth` and parent's kid count is leq `Trait::MaxKids`)
* Only the account that registered the Tree can revoke it, triggering recursion to delete all subtrees. To disincentivize expensive recursion, actions for adding members and adding subtrees require collateral in proportion to the marginal contribution of each action to worst case deletion complexity.
    * Collateral requirements for adding new members scale linearly with group size. 
    * Collateral requirements for adding new subtrees scales exponentially with number of children and depth.