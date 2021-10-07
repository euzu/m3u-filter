import React from "react";
import './playlist-filter.scss';

interface PlaylistFilterProps {
   onFilter: (filter: string) => void;
}

export default function PlaylistFilter(props: PlaylistFilterProps) {


    return <div className={'playlist-filter'}></div>
}