Many DB queries could be cached. The cache should be busted when the database
(or even select tables) is mutated. Only do this if there is an elegant solution
and we don't have to touch every file in the code base.
