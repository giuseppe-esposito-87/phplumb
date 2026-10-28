<?php

namespace App;

function twice(\Ds\Map $map, \Couchbase\Cluster $cluster): mixed
{
    return $map->get('x');
}
