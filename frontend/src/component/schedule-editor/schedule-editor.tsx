import React, {JSX, useCallback, useEffect, useMemo, useState} from "react";
import "./schedule-editor.scss";
import {Schedule, SourceConfig} from "../../model/server-config";
import TagSelect from "../tag-select/tags-select";
import {getIconByName} from "../../icons/icons";

interface ScheduleEditorProps {
  name: string;
  values: Schedule[],
  sources: SourceConfig[];
  onChange: (name: string, values: Schedule[]) => void;
}

export default function ScheduleEditor(props: ScheduleEditorProps) {
  const { name, values, sources, onChange} = props;
  const targets = useMemo(() => sources.flatMap(s => s.targets)
      .filter(Boolean).flatMap(t => ({value: t.name, label: t.name})), [sources]);
  const [schedules, setSchedules]  =   useState([]);

  useEffect(() => {
    const schedulesCopy = values?.map(s => ({...s}))  || [];
    setSchedules(schedulesCopy);
  }, [values])

  const handleTargetChange = useCallback((field: string, selected: any):void => {
    setSchedules(data => {
        data[field as any].targets = selected;
        onChange(name, data);
        return data;
    });
  }, [name, onChange]);

  const handleScheduleChange = useCallback((evt: any) => {
    const field = evt.target.dataset.field;
    setSchedules(data => {
      data[field as any].schedule = evt.target.value;
      onChange(name, data);
      return data;
    });

  }, [name, onChange]);

  const handleScheduleRemove = useCallback((evt: any) => {
    const index = evt.target.dataset.index;
    setSchedules(data => {
      data.splice(index, 1);
      let newData = [...data];
      onChange(name, newData);
      return newData;
    });
  }, [name, onChange]);

  const handleScheduleAdd = useCallback((evt: any) => {
    setSchedules(data => {
      let newData = [...data, {schedule: '', targets: undefined}];
      onChange(name, newData);
      return newData;
    });
  }, [name, onChange]);

  let lastIndex = (schedules?.length ?? 0) -1;

  const renderSchedule = useCallback((schedule: Schedule, index: number): JSX.Element => {
    return <div key={schedule.schedule + index} className="schedule-editor__schedule">
      <input defaultValue={schedule.schedule} data-field={index} onChange={handleScheduleChange}></input>
      <TagSelect options={targets} name={index + ''} defaultValues={schedule.targets || []} multi={true}
                 onSelect={handleTargetChange}></TagSelect>
      <div className={"schedule-editor__schedule-toolbar"}>
      <button title={'Delete'} data-index={index} onClick={handleScheduleRemove}>{getIconByName('ScheduleRemove')}</button>
        {index === lastIndex &&
            <button title={'Add'} onClick={handleScheduleAdd}>{getIconByName('ScheduleAdd')}</button>}
      </div>
    </div>
  }, [targets, handleScheduleChange, handleTargetChange, handleScheduleRemove, handleScheduleAdd, lastIndex]);

  return (
      <div className="schedule-editor">
        {schedules?.map(renderSchedule)}
        {lastIndex < 0 &&
          <button title={'Add'} onClick={handleScheduleAdd}>{getIconByName('ScheduleAdd')}</button>}
      </div>
  );
}
